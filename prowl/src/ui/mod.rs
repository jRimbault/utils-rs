//! UI rendering entry point.
//!
//! Splits the terminal frame into a fixed-height header panel and a
//! fill-height tree table, then delegates each region to its module.

mod header;
mod tree;

use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

/// Render a full terminal frame.
///
/// `app` is mutably borrowed because the tree renderer updates
/// `app.visible_rows` so scroll synchronisation in `App` knows how many
/// rows are currently on screen.
pub fn render(frame: &mut Frame, app: &mut App) {
    let [header_area, tree_area] =
        Layout::vertical([Constraint::Length(9), Constraint::Fill(1)]).areas(frame.area());

    header::render_header(frame, app, header_area);
    tree::render_tree(frame, app, tree_area);
}
