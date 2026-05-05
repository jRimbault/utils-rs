//! Interactive process picker TUI.
//!
//! Presents a full-terminal list of all running processes with real-time
//! filtering, allowing the user to select a PID without knowing it in advance.
//!
//! Architecture: this module is entirely imperative-shell — it does procfs I/O
//! to enumerate processes, drives a ratatui rendering loop, and handles
//! crossterm events.  The one pure function is `filter`, which is tested
//! independently.

use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEventKind, KeyModifiers, read},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, List, ListItem, ListState, Paragraph},
};

use crate::process;

// ---------------------------------------------------------------------------
// RAII guard — restores terminal state unconditionally on drop, mirroring
// the pattern used by `TerminalGuard` in main.rs.
// ---------------------------------------------------------------------------

struct PickerGuard;

impl Drop for PickerGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, cursor::Show);
    }
}

// ---------------------------------------------------------------------------
// Pure filtering logic — functional core, no I/O.
// ---------------------------------------------------------------------------

/// Return the subset of `all` whose display string contains `query`
/// (case-insensitive).  Preserves the PID-ascending order of the source slice.
pub fn filter(all: &[(process::Pid, String)], query: &str) -> Vec<(process::Pid, String)> {
    if query.is_empty() {
        return all.to_vec();
    }
    // Allocate the lowercase query once rather than per entry.
    let lower = query.to_lowercase();
    all.iter()
        .filter(|(_, display)| display.to_lowercase().contains(&lower))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Process enumeration — imperative shell, procfs I/O.
// ---------------------------------------------------------------------------

/// Enumerate all visible processes, returning a list of `(Pid, display_string)`
/// sorted ascending by PID.  Processes that return errors are silently skipped.
fn enumerate_processes() -> Vec<(process::Pid, String)> {
    let Ok(all) = procfs::process::all_processes() else {
        return Vec::new();
    };

    let mut entries: Vec<(process::Pid, String)> = all
        .filter_map(|r| r.ok())
        .filter_map(|proc| {
            let pid = proc.pid();
            // stat().comm gives the short name (≤15 chars); needed even when cmdline
            // is available so we have a fallback.
            let comm = proc.stat().ok()?.comm;
            // cmdline returns the full argv joined by NUL; use comm as fallback when
            // the process has no cmdline (kernel threads, or permission errors).
            let cmdline = proc
                .cmdline()
                .ok()
                .filter(|v| !v.is_empty())
                .map(|v| v.join(" "))
                .unwrap_or_else(|| comm.clone());
            let display = format!("{pid}  {cmdline}");
            Some((process::Pid::new(pid), display))
        })
        .collect();

    // Stable PID-ascending sort so filtering preserves relative order.
    entries.sort_by_key(|(pid, _)| pid.get());
    entries
}

// ---------------------------------------------------------------------------
// Picker state — kept in local variables within `pick` rather than a struct,
// since there is only one call site and the state does not escape the function.
// ---------------------------------------------------------------------------

/// Run the interactive picker and return the selected `Pid`, or `None` if the
/// user cancelled.
///
/// Enters raw mode and alternate screen for the duration; the `PickerGuard`
/// ensures the terminal is restored even on early `?` returns.
pub fn pick() -> anyhow::Result<Option<process::Pid>> {
    let all = enumerate_processes();

    let mut query = String::new();
    let mut matches = all.clone();
    // Index into `matches` for the highlighted row.
    let mut selected: usize = 0;

    terminal::enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    // Guard restores terminal on drop regardless of how `pick` returns.
    let _guard = PickerGuard;

    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    loop {
        // Clamp selection into the valid range each frame in case filtering
        // has shrunk the match list since the last keypress.
        if !matches.is_empty() {
            selected = selected.min(matches.len() - 1);
        } else {
            selected = 0;
        }

        let sel = selected;
        let q = query.clone();
        let m = matches.clone();

        terminal.draw(|frame| render_picker(frame, &q, &m, sel))?;

        // Blocking read — no async runtime needed for the picker.
        match read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    // --- Confirm selection ---
                    KeyCode::Enter => {
                        return Ok(matches.get(selected).map(|(pid, _)| *pid));
                    }

                    // --- Cancel ---
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('q') if key.modifiers.is_empty() => return Ok(None),

                    // --- Navigation ---
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !matches.is_empty() {
                            selected = (selected + 1).min(matches.len() - 1);
                        }
                    }

                    // --- Query editing ---
                    KeyCode::Backspace => {
                        query.pop();
                        matches = filter(&all, &query);
                        selected = 0;
                    }
                    KeyCode::Char(c) => {
                        // Ctrl+C / Ctrl+D pass-through to allow hard exit.
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            return Ok(None);
                        }
                        query.push(c);
                        matches = filter(&all, &query);
                        selected = 0;
                    }

                    _ => {}
                }
            }
            // Resize events require a redraw; the loop will handle this naturally.
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Pure rendering — functional core, no I/O other than writing to the frame.
// ---------------------------------------------------------------------------

/// Render the picker TUI into the provided frame.
///
/// Layout (top to bottom):
///   - 1 row: prompt line with current query text
///   - fill: scrollable list of filtered matches
///   - 1 row: footer with key hints
fn render_picker(
    frame: &mut Frame,
    query: &str,
    matches: &[(process::Pid, String)],
    selected: usize,
) {
    let [prompt_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    // Prompt row: "> " prefix + query text + cursor block.
    let prompt = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::new().fg(Color::Cyan).bold()),
        Span::styled(query.to_owned(), Style::new().fg(Color::White).bold()),
        // Block cursor indicator — a filled space that mimics a terminal cursor.
        Span::styled(" ", Style::new().bg(Color::White)),
    ]));
    frame.render_widget(prompt, prompt_area);

    // Build list items; highlight the selected row.
    let items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .map(|(i, (pid, display))| {
            // Separate the PID from the rest of the display string for aligned colouring.
            let pid_str = format!("{pid:>7}  ");
            let cmd_str = display
                .split_once("  ")
                .map(|x| x.1)
                .unwrap_or(display.as_str())
                .to_owned();

            let style = if i == selected {
                Style::new()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            };

            ListItem::new(Line::from(vec![
                Span::styled(pid_str, Style::new().fg(Color::Yellow).patch(style)),
                Span::styled(cmd_str, style),
            ]))
        })
        .collect();

    let count_title = format!(" {} matches ", matches.len());
    let list = List::new(items)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::DarkGray))
                .title(Span::styled(
                    count_title,
                    Style::new().fg(Color::Cyan).bold(),
                )),
        )
        .highlight_style(Style::new().bg(Color::DarkGray).bold());

    // Use ListState so ratatui handles scroll offset automatically, keeping
    // the selected item visible without manual offset arithmetic.
    let mut list_state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    // Footer with key hints in dim style.
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  type to filter  ", Style::new().fg(Color::DarkGray)),
        Span::styled("↑↓/jk", Style::new().fg(Color::White).bold()),
        Span::styled(" navigate  ", Style::new().fg(Color::DarkGray)),
        Span::styled("Enter", Style::new().fg(Color::White).bold()),
        Span::styled(" select  ", Style::new().fg(Color::DarkGray)),
        Span::styled("Esc", Style::new().fg(Color::White).bold()),
        Span::styled(" cancel", Style::new().fg(Color::DarkGray)),
    ]));
    frame.render_widget(footer, footer_area);
}

// ---------------------------------------------------------------------------
// Tests — cover only the pure `filter` function; TUI rendering is excluded.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: i32) -> process::Pid {
        process::Pid::new(n)
    }

    fn sample_entries() -> Vec<(process::Pid, String)> {
        vec![
            (pid(1), "1  systemd".to_owned()),
            (pid(42), "42  bash".to_owned()),
            (pid(100), "100  cargo run".to_owned()),
            (pid(200), "200  python3 script.py".to_owned()),
            (pid(999), "999  UPPERCASE_PROC".to_owned()),
        ]
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let all = sample_entries();
        let result = filter(&all, "");
        assert_eq!(result.len(), all.len());
    }

    #[test]
    fn filter_exact_match() {
        let all = sample_entries();
        let result = filter(&all, "bash");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, pid(42));
    }

    #[test]
    fn filter_case_insensitive() {
        let all = sample_entries();
        // "uppercase_proc" should match the entry with "UPPERCASE_PROC".
        let result = filter(&all, "uppercase_proc");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, pid(999));
    }

    #[test]
    fn filter_partial_match() {
        let all = sample_entries();
        // "py" matches "python3" and also "script.py" — both in one entry.
        let result = filter(&all, "py");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, pid(200));
    }

    #[test]
    fn filter_pid_number_as_query() {
        let all = sample_entries();
        // Matching by PID prefix in the display string.
        let result = filter(&all, "100");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, pid(100));
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let all = sample_entries();
        let result = filter(&all, "zzznomatch");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_preserves_pid_ascending_order() {
        let all = sample_entries();
        // "a" matches systemd, bash, cargo, python3 — all except UPPERCASE_PROC
        // (which does contain no 'a' wait — "UPPERCASE" has no 'a' but the display
        // string is "999  UPPERCASE_PROC", let's use "system" to be precise).
        let result = filter(&all, "1");
        // "1" appears in "1  systemd" (pid 1) and "100  cargo run" (pid 100).
        assert!(result.len() >= 2);
        let pids: Vec<i32> = result.iter().map(|(p, _)| p.get()).collect();
        let mut sorted = pids.clone();
        sorted.sort();
        assert_eq!(pids, sorted, "results must be in PID-ascending order");
    }

    #[test]
    fn filter_multi_word_query() {
        let all = sample_entries();
        let result = filter(&all, "cargo run");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, pid(100));
    }
}
