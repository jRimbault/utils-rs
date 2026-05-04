//! Tree-table widget — htop-style columnar view of the process tree.
//!
//! Renders the flattened `FlatRow` list from `App` as a ratatui `Table` with
//! colour-coded CPU and memory bars.  The function mutates `app.visible_rows`
//! so that `App::sync_scroll` can keep the selection visible on the next tick.

use crate::{app::App, format};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

pub fn render_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    // Inform App how many rows fit so sync_scroll can page correctly.
    // Subtract 3 for top border + column header row + bottom border.
    app.visible_rows = (area.height as usize).saturating_sub(3);

    let header = Row::new(vec![
        Cell::new(Line::from("PID").centered()),
        Cell::new("USER"),
        Cell::new("S"),
        Cell::new("CPU%"),
        Cell::new("CPU"),
        Cell::new("MEM%"),
        Cell::new("MEM"),
        Cell::new("RES"),
        Cell::new("Command"),
    ])
    .style(Style::new().dim());

    let rows: Vec<Row> = app
        .flat_rows
        .iter()
        .enumerate()
        .map(|(i, fr)| {
            let is_selected = i == app.selected;
            let cpu_color = format::intensity_color(fr.cpu_pct);
            let mem_color = format::intensity_color(fr.mem_pct);
            let cpu_bar = format::bar(fr.cpu_pct / 100.0, 8);
            let mem_bar = format::bar(fr.mem_pct / 100.0, 8);
            let cmd = format!("{}{}", fr.connector, fr.cmdline);

            // Selection overrides thread dimming so the selected row is always visible.
            let base_style = if is_selected {
                Style::new()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else if fr.is_thread {
                Style::new().add_modifier(Modifier::DIM)
            } else {
                Style::new()
            };

            Row::new(vec![
                Cell::new(format!("{:>7}", fr.pid)),
                Cell::new(format!("{:<8}", fr.user)),
                Cell::new(format!(" {} ", fr.state)),
                Cell::new(format!("{:>4.1}", fr.cpu_pct)).style(Style::new().fg(cpu_color)),
                Cell::new(cpu_bar).style(Style::new().fg(cpu_color)),
                Cell::new(format!("{:>4.1}", fr.mem_pct)).style(Style::new().fg(mem_color)),
                Cell::new(mem_bar).style(Style::new().fg(mem_color)),
                Cell::new(format!("{:>7}", format::format_bytes(fr.mem_rss_bytes))),
                Cell::new(cmd),
            ])
            .style(base_style)
        })
        .collect();

    let widths = [
        Constraint::Length(8),  // PID
        Constraint::Length(9),  // USER
        Constraint::Length(3),  // S
        Constraint::Length(5),  // CPU%
        Constraint::Length(10), // CPU bar
        Constraint::Length(5),  // MEM%
        Constraint::Length(10), // MEM bar
        Constraint::Length(8),  // RES
        Constraint::Fill(1),    // Command
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::DarkGray)),
        )
        .row_highlight_style(Style::new().bg(Color::DarkGray).bold())
        .column_spacing(1);

    frame.render_stateful_widget(table, area, &mut app.table_state);
}
