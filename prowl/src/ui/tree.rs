//! Tree-table widget — htop-style columnar view of the process tree.
//!
//! Renders the flattened `FlatRow` list from `App` as a ratatui `Table` with
//! colour-coded CPU and memory bars.  The function mutates `app.visible_rows`
//! so that `App::sync_scroll` can keep the selection visible on the next tick.
//!
//! At narrow terminal widths, lower-priority columns are progressively hidden
//! so the Command column always gets a reasonable amount of space.

use crate::{app::App, format};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

/// Which columns are visible at the current terminal width.
struct ColumnSet {
    user: bool,
    state_full: bool, // full word vs single char
    cpu_bar: bool,
    mem_pct: bool,
    mem_bar: bool,
    res: bool,
    elapsed: bool,
}

impl ColumnSet {
    /// Pick the richest column set that leaves at least `min_cmd` columns for Command.
    fn for_width(w: u16) -> Self {
        // Fixed overhead: PID(8) + spacing(1 per column) + CPU%(5) + Command(Fill).
        // Calculate available width minus the always-shown columns.
        let base = 8 + 5; // PID + CPU%
        let remaining = w.saturating_sub(base + 4); // borders + min spacing

        // Progressive tiers — each tier adds columns from the widest layout.
        if remaining >= 75 {
            // Full layout: USER(9) STATE(11) CPU_BAR(10) MEM%(5) MEM_BAR(10) RES(8) ELAPSED(9)
            Self { user: true, state_full: true, cpu_bar: true, mem_pct: true, mem_bar: true, res: true, elapsed: true }
        } else if remaining >= 56 {
            // Drop ELAPSED and MEM bar
            Self { user: true, state_full: true, cpu_bar: true, mem_pct: true, mem_bar: false, res: true, elapsed: false }
        } else if remaining >= 40 {
            // Drop CPU bar, RES, abbreviate state
            Self { user: true, state_full: false, cpu_bar: false, mem_pct: true, mem_bar: false, res: false, elapsed: false }
        } else if remaining >= 25 {
            // Drop USER, MEM%
            Self { user: false, state_full: false, cpu_bar: false, mem_pct: false, mem_bar: false, res: false, elapsed: false }
        } else {
            // Minimal: PID + CPU% + Command
            Self { user: false, state_full: false, cpu_bar: false, mem_pct: false, mem_bar: false, res: false, elapsed: false }
        }
    }
}

pub fn render_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    // Subtract 3 for top border + column header row + bottom border.
    app.visible_rows = (area.height as usize).saturating_sub(3);

    // Inner width minus left/right borders.
    let inner_width = area.width.saturating_sub(2);
    let cols = ColumnSet::for_width(inner_width);

    let mut header_cells: Vec<Cell> = vec![Cell::new(Line::from("PID").centered())];
    let mut widths: Vec<Constraint> = vec![Constraint::Length(8)];

    if cols.user {
        header_cells.push(Cell::new("USER"));
        widths.push(Constraint::Length(9));
    }
    if cols.state_full {
        header_cells.push(Cell::new("STATE"));
        widths.push(Constraint::Length(11));
    } else {
        header_cells.push(Cell::new("S"));
        widths.push(Constraint::Length(3));
    }
    header_cells.push(Cell::new("CPU%"));
    widths.push(Constraint::Length(5));
    if cols.cpu_bar {
        header_cells.push(Cell::new("CPU"));
        widths.push(Constraint::Length(10));
    }
    if cols.mem_pct {
        header_cells.push(Cell::new("MEM%"));
        widths.push(Constraint::Length(5));
    }
    if cols.mem_bar {
        header_cells.push(Cell::new("MEM"));
        widths.push(Constraint::Length(10));
    }
    if cols.res {
        header_cells.push(Cell::new("RES"));
        widths.push(Constraint::Length(8));
    }
    if cols.elapsed {
        header_cells.push(Cell::new("ELAPSED"));
        widths.push(Constraint::Length(9));
    }
    header_cells.push(Cell::new("Command"));
    widths.push(Constraint::Fill(1));

    let header = Row::new(header_cells).style(Style::new().fg(Color::White));

    let rows: Vec<Row> = app
        .flat_rows
        .iter()
        .enumerate()
        .map(|(i, fr)| {
            let is_selected = i == app.selected;
            let cpu_color = format::intensity_color(fr.cpu_pct);
            let mem_color = format::intensity_color(fr.mem_pct);

            let collapse_marker = if fr.has_children {
                if fr.is_collapsed { "▸ " } else { "▾ " }
            } else {
                ""
            };
            let cmd = format!("{}{}{}", fr.connector, collapse_marker, fr.cmdline);

            let base_style = if is_selected {
                Style::new()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if fr.is_thread {
                Style::new().add_modifier(Modifier::DIM)
            } else {
                Style::new()
            };

            let is_thread = fr.is_thread;
            let mut cells: Vec<Cell> = vec![Cell::new(format!("{:>7}", fr.pid))];

            if cols.user {
                cells.push(Cell::new(if is_thread { String::new() } else { format!("{:<8}", fr.user) }));
            }
            if cols.state_full {
                cells.push(Cell::new(format!(" {:<9}", format::state_word(fr.state))));
            } else {
                cells.push(Cell::new(format!(" {} ", fr.state)));
            }
            cells.push(Cell::new(format!("{:>4.1}", fr.cpu_pct)).style(Style::new().fg(cpu_color)));
            if cols.cpu_bar {
                cells.push(Cell::new(format::bar(fr.cpu_pct / 100.0, 8)).style(Style::new().fg(cpu_color)));
            }
            if cols.mem_pct {
                cells.push(
                    Cell::new(if is_thread { String::new() } else { format!("{:>4.1}", fr.mem_pct) })
                        .style(Style::new().fg(mem_color)),
                );
            }
            if cols.mem_bar {
                cells.push(
                    Cell::new(if is_thread { String::new() } else { format::bar(fr.mem_pct / 100.0, 8) })
                        .style(Style::new().fg(mem_color)),
                );
            }
            if cols.res {
                cells.push(Cell::new(if is_thread { String::new() } else { format!("{:>7}", format::format_bytes(fr.mem_rss_bytes)) }));
            }
            if cols.elapsed {
                cells.push(Cell::new(if is_thread { String::new() } else { format!("{:>8}", format::format_duration(fr.elapsed)) }));
            }
            cells.push(Cell::new(cmd));

            Row::new(cells).style(base_style)
        })
        .collect();

    let footer_hints = Line::from(vec![
        Span::styled(" q", Style::new().fg(Color::White).bold()),
        Span::styled(" quit ", Style::new().fg(Color::DarkGray)),
        Span::styled("↑↓/jk", Style::new().fg(Color::White).bold()),
        Span::styled(" nav ", Style::new().fg(Color::DarkGray)),
        Span::styled("⏎/␣", Style::new().fg(Color::White).bold()),
        Span::styled(" collapse ", Style::new().fg(Color::DarkGray)),
        Span::styled("t", Style::new().fg(Color::White).bold()),
        Span::styled(" threads ", Style::new().fg(Color::DarkGray)),
    ]);

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::DarkGray))
                .title_bottom(footer_hints),
        )
        .row_highlight_style(Style::new().bg(Color::DarkGray).bold())
        .column_spacing(1);

    frame.render_stateful_widget(table, area, &mut app.table_state);
}
