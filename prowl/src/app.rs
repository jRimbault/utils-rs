//! Application state — pure UI coordination layer.
//!
//! `App` holds only what the renderer needs: the latest process snapshot,
//! rolling metric history for sparklines, selection/scroll state, and display
//! preferences.  All data collection lives in `collector`; all rendering in `ui`.

use std::collections::{HashSet, VecDeque};

use crate::{
    format::Percent,
    process::{Node, Pid, Tree},
    tree::{Row, flatten},
};

const HISTORY_CAPACITY: usize = 200;

/// Bounded ring buffer of `Percent` samples for sparkline graphs.
pub struct History {
    values: VecDeque<Percent>,
    capacity: usize,
}

impl History {
    fn new(capacity: usize) -> Self {
        Self {
            values: VecDeque::new(),
            capacity,
        }
    }

    pub fn push(&mut self, p: Percent) {
        self.values.push_back(p);
        if self.values.len() > self.capacity {
            self.values.pop_front();
        }
    }

    /// Iterate over stored samples oldest-first, yielding `Percent` by value.
    pub fn iter(&self) -> impl Iterator<Item = Percent> + '_ {
        self.values.iter().copied()
    }

    // Provided as part of the bounded-buffer API; not currently called by renderers.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

pub struct App {
    root: Option<Tree>,
    /// Name of the observed root process; displayed in the header title.
    name: String,
    flat_rows: Vec<Row>,
    /// Index into `flat_rows` for keyboard selection.
    selected: usize,
    /// Ratatui stateful widget state (carries scroll offset).
    table_state: ratatui::widgets::TableState,
    show_threads: bool,
    /// Set to `true` when the monitored PID has disappeared.
    exited: bool,
    /// Number of tree rows currently visible on screen; set by the renderer.
    visible_rows: usize,
    /// PIDs whose subtrees are collapsed in the tree view — purely internal.
    collapsed: HashSet<Pid>,
    /// CPU% per sample — feeds the header sparkline.
    cpu_history: History,
    /// MEM% per sample — feeds the header sparkline.
    mem_history: History,
}

impl App {
    pub fn new(show_threads: bool) -> Self {
        Self {
            root: None,
            name: String::new(),
            flat_rows: Vec::new(),
            selected: 0,
            table_state: ratatui::widgets::TableState::default(),
            show_threads,
            exited: false,
            visible_rows: 20,
            collapsed: HashSet::new(),
            cpu_history: History::new(HISTORY_CAPACITY),
            mem_history: History::new(HISTORY_CAPACITY),
        }
    }

    // --- Read accessors ---

    pub fn root(&self) -> Option<&Node> {
        self.root.as_ref().and_then(|t| t.root())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn flat_rows(&self) -> &[Row] {
        &self.flat_rows
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    // Provided as part of the display-preferences API; may be used by future renderers.
    #[allow(dead_code)]
    pub fn show_threads(&self) -> bool {
        self.show_threads
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    pub fn cpu_history(&self) -> &History {
        &self.cpu_history
    }

    pub fn mem_history(&self) -> &History {
        &self.mem_history
    }

    // --- Mutable accessors for ratatui stateful widget rendering ---

    pub fn table_state_mut(&mut self) -> &mut ratatui::widgets::TableState {
        &mut self.table_state
    }

    /// Update how many tree rows are visible on screen.
    ///
    /// Called by the tree renderer each frame before `sync_scroll` runs.
    pub fn set_visible_rows(&mut self, n: usize) {
        self.visible_rows = n;
    }

    // --- State mutation methods ---

    /// Replace the current snapshot with a freshly collected one.
    pub fn apply_snapshot(&mut self, tree: Tree) {
        let Some(root) = tree.root() else { return };
        self.name = root.name().to_owned();
        self.cpu_history.push(root.cpu_pct());
        self.mem_history.push(root.mem_pct());
        self.flat_rows = flatten(&tree, self.show_threads, &self.collapsed);
        if !self.flat_rows.is_empty() && self.selected >= self.flat_rows.len() {
            self.selected = self.flat_rows.len() - 1;
        }
        self.root = Some(tree);
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
        if let Some(tree) = &self.root {
            self.flat_rows = flatten(tree, self.show_threads, &self.collapsed);
        }
        if !self.flat_rows.is_empty() && self.selected >= self.flat_rows.len() {
            self.selected = self.flat_rows.len() - 1;
        }
        self.sync_scroll();
    }

    /// Toggle collapse state of the currently selected node's subtree.
    pub fn toggle_collapse(&mut self) {
        if let Some(row) = self.flat_rows.get(self.selected) {
            let pid = row.pid();
            if !self.collapsed.remove(&pid) {
                self.collapsed.insert(pid);
            }
            if let Some(tree) = &self.root {
                self.flat_rows = flatten(tree, self.show_threads, &self.collapsed);
            }
            if !self.flat_rows.is_empty() && self.selected >= self.flat_rows.len() {
                self.selected = self.flat_rows.len() - 1;
            }
            self.sync_scroll();
        }
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
