//! Header panel — btop-style detail view for the root process.
//!
//! Layout (inner area, 7 rows for a Length(9) outer block):
//!
//!   row 0        │ PID + name                    [key hints]
//!   rows 1-5     │ [CPU graph panel]  │  [info panel]
//!   row 6        │ full command line
//!
//! The CPU panel renders a tall multi-row braille graph (5 rows = 20 vertical
//! levels) with a narrow label column showing the current percentage and the
//! vertical "C P U" label.  The info panel shows status, IO rates, memory, and
//! parent/user on separate lines.

use crate::{app::App, format};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let Some(root) = &app.root else {
        frame.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::DarkGray))
                .title(Span::styled(" pidtree ", Style::new().fg(Color::Red).bold())),
            area,
        );
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray))
        .title(Span::styled(" pidtree ", Style::new().fg(Color::Red).bold()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [top_row, main_area, cmd_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    // Row 0: PID + name left, key-bind hints right.
    let hints = " [q]quit [↑↓/jk]nav [t]threads ";
    let pid_name = format!(" {:>7}  {}", root.pid, root.name);
    let pad = inner
        .width
        .saturating_sub(pid_name.len() as u16 + hints.len() as u16);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(pid_name, Style::new().bold().fg(Color::White)),
            Span::raw(" ".repeat(pad as usize)),
            Span::styled(hints, Style::new().fg(Color::DarkGray)),
        ])),
        top_row,
    );

    // Rows 1-5: CPU graph panel (left) | info panel (right).
    let [cpu_panel, info_panel] = Layout::horizontal([
        Constraint::Percentage(35),
        Constraint::Fill(1),
    ])
    .areas(main_area);

    render_cpu_panel(frame, cpu_panel, &app.cpu_history, root.cpu_pct);
    render_info_panel(frame, info_panel, root, &app.mem_history);

    // Row 6: full command line spanning both panels.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            label("CMD: "),
            Span::styled(root.cmdline.clone(), Style::new().fg(Color::White)),
        ])),
        cmd_row,
    );
}

/// Render the left CPU panel: narrow label column + tall braille graph.
///
/// Row 0 shows the current percentage; rows 1-3 show the "C", "P", "U" label
/// letters; remaining rows are blank.  The graph fills all rows from the bottom
/// upward so the trace reads as a filled area chart.
fn render_cpu_panel(
    frame: &mut Frame,
    area: Rect,
    history: &std::collections::VecDeque<u64>,
    cpu_pct: f64,
) {
    if area.width < 4 || area.height == 0 {
        return;
    }
    const LABEL_W: u16 = 7;
    let [label_col, graph_col] = Layout::horizontal([
        Constraint::Length(LABEL_W),
        Constraint::Fill(1),
    ])
    .areas(area);

    let rows = graph_col.height as usize;
    let graph_rows = braille_graph_multi(history, graph_col.width as usize, rows);
    let cpu_color = format::intensity_color(cpu_pct);
    const ROW_LABELS: [&str; 4] = ["", "C", "P", "U"];

    for r in 0..rows {
        let y = area.top() + r as u16;
        if y >= area.bottom() {
            break;
        }

        let label_text = if r == 0 {
            format!("{:>5.1}% ", cpu_pct)
        } else {
            format!("{:<7}", ROW_LABELS.get(r).copied().unwrap_or(""))
        };
        let label_style = if r == 0 {
            Style::new().fg(cpu_color).bold()
        } else {
            Style::new().fg(Color::Red)
        };
        frame.render_widget(
            Paragraph::new(Span::styled(label_text, label_style)),
            Rect::new(label_col.left(), y, label_col.width, 1),
        );

        if let Some(row_str) = graph_rows.get(r) {
            frame.render_widget(
                Paragraph::new(Span::styled(row_str.as_str(), Style::new().fg(cpu_color))),
                Rect::new(graph_col.left(), y, graph_col.width, 1),
            );
        }
    }
}

/// Render the right info panel: status/IO/memory/parent rows.
fn render_info_panel(
    frame: &mut Frame,
    area: Rect,
    root: &crate::process::ProcessNode,
    mem_history: &std::collections::VecDeque<u64>,
) {
    let h = area.height as usize;
    if h == 0 {
        return;
    }

    // Row layout:
    //   0 — blank (visual gap matching btop's right-panel padding)
    //   1 — Status + Elapsed + IO/R + IO/W
    //   2 — Parent + User
    //   3 — Memory braille graph
    //   4+ — blank
    let static_rows: &[Option<Line>] = &[
        Some(Line::raw("")),
        Some(Line::from(vec![
            Span::raw("  "),
            label("Status: "),
            value(&format!("{:<10}", root.state)),
            label("Elapsed: "),
            value(&format!("{:<12}", format::format_duration(root.elapsed))),
            label("IO/R: "),
            value(&format!("{}/s  ", format::format_bytes(root.io_read_rate))),
            label("IO/W: "),
            value(&format!("{}/s", format::format_bytes(root.io_write_rate))),
        ])),
        Some(Line::from(vec![
            Span::raw("  "),
            label("Parent: "),
            value(&format!("{:<15}", root.parent_name)),
            label("User: "),
            value(&root.user),
        ])),
        None, // memory row — rendered via render_sparkline_row
    ];

    for r in 0..h {
        let y = area.top() + r as u16;
        let row_rect = Rect::new(area.left(), y, area.width, 1);
        match static_rows.get(r) {
            Some(Some(line)) => {
                frame.render_widget(Paragraph::new(line.clone()), row_rect);
            }
            Some(None) => {
                // Memory graph row
                render_sparkline_row(
                    frame,
                    row_rect,
                    &format!("  Mem {:>6.1}%  ", root.mem_pct),
                    mem_history,
                    Color::Green,
                    &format!("  {}", format::format_bytes(root.mem_rss_bytes)),
                );
            }
            None => {} // rows beyond the defined content: leave blank
        }
    }
}

/// Render a 1-row braille graph between a left label and a right label.
fn render_sparkline_row(
    frame: &mut Frame,
    area: Rect,
    left_label: &str,
    history: &std::collections::VecDeque<u64>,
    color: Color,
    right_label: &str,
) {
    let right_width = right_label.len() as u16;
    let left_width = left_label.len() as u16;

    let [left, spark, right] = Layout::horizontal([
        Constraint::Length(left_width),
        Constraint::Fill(1),
        Constraint::Length(right_width),
    ])
    .areas(area);

    frame.render_widget(
        Paragraph::new(Span::styled(left_label.to_owned(), Style::new().fg(Color::Red))),
        left,
    );

    let graph = braille_graph(history, spark.width as usize);
    frame.render_widget(
        Paragraph::new(Span::styled(graph, Style::new().fg(color))),
        spark,
    );

    if !right_label.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                right_label.to_owned(),
                Style::new().fg(Color::DarkGray),
            )),
            right,
        );
    }
}

/// Build a single-row braille time-series string for `width` terminal columns.
///
/// Each column encodes two consecutive samples (left/right braille dot columns)
/// mapped to 0-4 filled dot rows from the bottom.  The most-recent sample sits
/// at the rightmost column; the trace grows right-to-left as history fills in.
fn braille_graph(history: &std::collections::VecDeque<u64>, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let max = history.iter().copied().max().unwrap_or(0).max(1);
    let n = 2 * width;
    let take = n.min(history.len());
    let mut samples = vec![0u64; n];
    for (i, &v) in history.iter().rev().take(take).enumerate() {
        samples[n - 1 - i] = v;
    }
    (0..width)
        .map(|col| braille_cell(samples[2 * col], samples[2 * col + 1], max))
        .collect()
}

/// Build a multi-row braille filled-area graph as a `Vec` of row strings.
///
/// Index 0 is the topmost terminal row, index `rows-1` the bottom.  Each
/// terminal row contributes 4 braille dot levels, so `rows` rows yield
/// `rows × 4` distinct fill heights.  Values are scaled to the history maximum
/// and filled from the bottom upward, identical to btop's graph style.
fn braille_graph_multi(
    history: &std::collections::VecDeque<u64>,
    width: usize,
    rows: usize,
) -> Vec<String> {
    if width == 0 || rows == 0 {
        return vec![String::new(); rows];
    }
    let max = history.iter().copied().max().unwrap_or(0).max(1);
    let n = 2 * width;
    let take = n.min(history.len());
    let mut samples = vec![0u64; n];
    for (i, &v) in history.iter().rev().take(take).enumerate() {
        samples[n - 1 - i] = v;
    }

    let total_levels = (rows as u64) * 4;

    (0..rows)
        .map(|row| {
            // row 0 = topmost terminal row = highest fill levels
            let level_base = (rows - 1 - row) as u64 * 4;
            (0..width)
                .map(|col| {
                    let lv = (samples[2 * col]     * total_levels) / max;
                    let rv = (samples[2 * col + 1] * total_levels) / max;
                    let lh = lv.saturating_sub(level_base).min(4) as usize;
                    let rh = rv.saturating_sub(level_base).min(4) as usize;
                    braille_cell_heights(lh, rh)
                })
                .collect()
        })
        .collect()
}

/// Encode one braille cell from per-column fill heights in 0-4.
///
/// Unicode 8-dot braille fill-from-bottom bitmasks (U+2800 base):
/// ```text
/// Row 4 (bottom): left = bit6 (0x40), right = bit7 (0x80)
/// Row 3:          left = bit2 (0x04), right = bit5 (0x20)
/// Row 2:          left = bit1 (0x02), right = bit4 (0x10)
/// Row 1 (top):    left = bit0 (0x01), right = bit3 (0x08)
/// ```
fn braille_cell_heights(lh: usize, rh: usize) -> char {
    const LEFT:  [u8; 5] = [0x00, 0x40, 0x44, 0x46, 0x47];
    const RIGHT: [u8; 5] = [0x00, 0x80, 0xA0, 0xB0, 0xB8];
    char::from_u32(0x2800 | u32::from(LEFT[lh] | RIGHT[rh])).unwrap_or(' ')
}

fn braille_cell(left_val: u64, right_val: u64, max: u64) -> char {
    let lh = ((left_val  * 4) / max).min(4) as usize;
    let rh = ((right_val * 4) / max).min(4) as usize;
    braille_cell_heights(lh, rh)
}

fn label(s: &str) -> Span<'static> {
    Span::styled(s.to_owned(), Style::new().fg(Color::Red))
}

fn value(s: &str) -> Span<'static> {
    Span::styled(s.to_owned(), Style::new().fg(Color::White))
}
