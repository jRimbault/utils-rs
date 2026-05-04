//! Application state — pure UI coordination layer.
//!
//! `App` holds only what the renderer needs: the latest process snapshot,
//! rolling metric history for sparklines, selection/scroll state, and display
//! preferences.  All data collection lives in `collector`; all rendering in `ui`.

use crate::process::{FlatRow, ProcessNode, flatten};
use std::collections::VecDeque;

const HISTORY_CAPACITY: usize = 200;

pub struct App {
    pub root: Option<ProcessNode>,
    pub flat_rows: Vec<FlatRow>,
    /// Index into `flat_rows` for keyboard selection.
    pub selected: usize,
    /// Ratatui stateful widget state (carries scroll offset).
    pub table_state: ratatui::widgets::TableState,
    pub show_threads: bool,
    /// Set to `true` when the monitored PID has disappeared.
    pub exited: bool,
    /// Number of tree rows currently visible on screen; set by the renderer.
    pub visible_rows: usize,
    /// CPU% × 10 per sample — feeds the header sparkline.
    pub cpu_history: VecDeque<u64>,
    /// MEM% × 10 per sample — feeds the header sparkline.
    pub mem_history: VecDeque<u64>,
}

impl App {
    pub fn new(show_threads: bool) -> Self {
        Self {
            root: None,
            flat_rows: Vec::new(),
            selected: 0,
            table_state: ratatui::widgets::TableState::default(),
            show_threads,
            exited: false,
            visible_rows: 20,
            cpu_history: VecDeque::new(),
            mem_history: VecDeque::new(),
        }
    }

    /// Replace the current snapshot with a freshly collected one.
    pub fn apply_snapshot(&mut self, root: ProcessNode) {
        push_history(&mut self.cpu_history, (root.cpu_pct * 10.0) as u64);
        push_history(&mut self.mem_history, (root.mem_pct * 10.0) as u64);
        self.flat_rows = flatten(&root, self.show_threads);
        if !self.flat_rows.is_empty() && self.selected >= self.flat_rows.len() {
            self.selected = self.flat_rows.len() - 1;
        }
        self.root = Some(root);
        self.sync_scroll();
    }

    pub fn mark_exited(&mut self) {
        self.exited = true;
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.sync_scroll();
    }

    pub fn move_down(&mut self) {
        if !self.flat_rows.is_empty() {
            self.selected = (self.selected + 1).min(self.flat_rows.len() - 1);
        }
        self.sync_scroll();
    }

    /// Toggle thread visibility using the already-cached snapshot — no refresh needed.
    pub fn toggle_threads(&mut self) {
        self.show_threads = !self.show_threads;
        if let Some(root) = &self.root {
            self.flat_rows = flatten(root, self.show_threads);
        }
        if !self.flat_rows.is_empty() && self.selected >= self.flat_rows.len() {
            self.selected = self.flat_rows.len() - 1;
        }
        self.sync_scroll();
    }

    fn sync_scroll(&mut self) {
        if self.visible_rows == 0 {
            return;
        }
        let offset = self.table_state.offset();
        if self.selected < offset {
            *self.table_state.offset_mut() = self.selected;
        } else if self.selected >= offset + self.visible_rows {
            *self.table_state.offset_mut() = self.selected + 1 - self.visible_rows;
        }
        self.table_state.select(Some(self.selected));
    }
}

fn push_history(hist: &mut VecDeque<u64>, value: u64) {
    hist.push_back(value);
    if hist.len() > HISTORY_CAPACITY {
        hist.pop_front();
    }
}
