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
//! process metadata on separate lines.

use crate::{
    app::{App, History},
    format,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " {} ",
        if app.name().is_empty() {
            "prowl"
        } else {
            app.name()
        }
    );
    let Some(root) = app.root() else {
        frame.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::DarkGray))
                .title(Span::styled(title, Style::new().fg(Color::White).bold())),
            area,
        );
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} ", root.name()),
            Style::new().fg(Color::White).bold(),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [_top_row, main_area, cmd_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    // Rows 1-5: CPU graph panel (left) | info panel (right).
    let [cpu_panel, info_panel] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Fill(1)]).areas(main_area);

    render_cpu_panel(frame, cpu_panel, app.cpu_history(), root.cpu_pct());
    render_info_panel(frame, info_panel, root, app.mem_history());

    // Row 6: full command line spanning both panels.
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            label("CMD: "),
            value(root.cmdline()),
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
    history: &History,
    cpu_pct: crate::format::Percent,
) {
    if area.width < 4 || area.height == 0 {
        return;
    }
    const LABEL_W: u16 = 7;
    let [label_col, graph_col] =
        Layout::horizontal([Constraint::Length(LABEL_W), Constraint::Fill(1)]).areas(area);

    let rows = graph_col.height as usize;
    // Decouple format from app: pass the iterator of Percent directly.
    let graph_rows = format::braille_graph_multi(history.iter(), graph_col.width as usize, rows);
    let cpu_color = cpu_pct.color();
    const ROW_LABELS: [&str; 4] = ["", "C", "P", "U"];

    for r in 0..rows {
        let y = area.top() + r as u16;
        if y >= area.bottom() {
            break;
        }

        let label_text = if r == 0 {
            format!("{:>5.1}% ", cpu_pct.value())
        } else {
            format!("{:<7}", ROW_LABELS.get(r).copied().unwrap_or(""))
        };
        let label_style = if r == 0 {
            Style::new().fg(cpu_color).bold()
        } else {
            Style::new().fg(Color::White)
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

/// Render the right info panel: status/IO/memory/process metadata rows.
fn render_info_panel(
    frame: &mut Frame,
    area: Rect,
    root: &crate::process::Node,
    mem_history: &History,
) {
    let h = area.height as usize;
    if h == 0 {
        return;
    }

    for r in 0..h {
        let y = area.top() + r as u16;
        let row_rect = Rect::new(area.left(), y, area.width, 1);
        match r {
            1 => render_info_fields(
                frame,
                row_rect,
                [
                    info_field("  ", "Status: ", format::state_word(root.state())),
                    info_field(" ", "Elapsed: ", format::format_duration(root.elapsed())),
                    info_field(
                        " ",
                        "IO/R: ",
                        format!("{}/s", format::format_bytes(root.io().read())),
                    ),
                    info_field(
                        " ",
                        "IO/W: ",
                        format!("{}/s", format::format_bytes(root.io().write())),
                    ),
                ],
            ),
            2 => render_info_fields(
                frame,
                row_rect,
                [
                    info_field("  ", "Parent: ", root.parent_name()),
                    info_field(" ", "User: ", root.user()),
                    info_field(" ", "Threads: ", root.thread_count().to_string()),
                    info_field(" ", "Subproc: ", root.subprocess_count().to_string()),
                ],
            ),
            3 => {
                render_sparkline_row(
                    frame,
                    row_rect,
                    &format!("  Mem {:>6.1}%  ", root.mem_pct().value()),
                    mem_history,
                    Color::Green,
                    &format!("  {}", format::format_bytes(root.mem_rss_bytes())),
                );
            }
            4 => {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::raw("  "),
                        label("CPU time: "),
                        value(&format::format_duration(root.cpu_time())),
                    ])),
                    row_rect,
                );
            }
            _ => {}
        }
    }
}

fn render_info_fields(frame: &mut Frame, area: Rect, fields: [Line<'static>; 4]) {
    let [col1, col2, col3, col4] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [field1, field2, field3, field4] = fields;

    frame.render_widget(Paragraph::new(field1), col1);
    frame.render_widget(Paragraph::new(field2), col2);
    frame.render_widget(Paragraph::new(field3), col3);
    frame.render_widget(Paragraph::new(field4), col4);
}

fn info_field(prefix: &str, name: &str, contents: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::raw(prefix.to_owned()),
        label(name),
        value(&contents.into()),
    ])
}

/// Render a 1-row braille graph between a left label and a right label.
fn render_sparkline_row(
    frame: &mut Frame,
    area: Rect,
    left_label: &str,
    history: &History,
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
        Paragraph::new(Span::styled(
            left_label.to_owned(),
            Style::new().fg(Color::White),
        )),
        left,
    );

    // Decouple format from app: pass the iterator of Percent directly.
    let graph = format::braille_graph(history.iter(), spark.width as usize);
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

fn label(s: &str) -> Span<'static> {
    Span::styled(s.to_owned(), Style::new().fg(Color::White))
}

fn value(s: &str) -> Span<'static> {
    Span::styled(s.to_owned(), Style::new().fg(Color::White).bold())
}
