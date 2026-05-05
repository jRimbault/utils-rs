//! Pure tree-flattening logic — functional core, no procfs imports.
//!
//! Converts a `process::Node` tree into an ordered list of `Row`s for the
//! table widget, computing connector glyphs and display command strings along
//! the way.

use std::{collections::HashSet, time::Duration};

use crate::{format::Percent, process::Pid};

/// Flattened row used by the tree-table widget.
///
/// `connector` contains the full Unicode-art prefix produced by `flatten`,
/// e.g. `"│  ├─ "`.
#[derive(Clone)]
pub struct Row {
    pid: Pid,
    /// Full tree-art prefix + connector glyph, ready to prepend to `cmdline`.
    connector: String,
    /// Full command line (argv joined by spaces); falls back to `stat.comm` for threads
    /// and kernel workers where `/proc/<pid>/cmdline` is empty.
    cmdline: String,
    user: String,
    state: char,
    cpu_pct: Percent,
    mem_pct: Percent,
    mem_rss_bytes: u64,
    elapsed: Duration,
    /// Total CPU time consumed (utime + stime from `/proc/<pid>/stat`).
    cpu_time: Duration,
    is_thread: bool,
    /// Whether this node has visible children (used for collapse indicator).
    has_children: bool,
    /// Whether this node's subtree is currently collapsed.
    is_collapsed: bool,
}

impl Row {
    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn connector(&self) -> &str {
        &self.connector
    }

    pub fn cmdline(&self) -> &str {
        &self.cmdline
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn state(&self) -> char {
        self.state
    }

    pub fn cpu_pct(&self) -> Percent {
        self.cpu_pct
    }

    pub fn mem_pct(&self) -> Percent {
        self.mem_pct
    }

    pub fn mem_rss_bytes(&self) -> u64 {
        self.mem_rss_bytes
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn cpu_time(&self) -> Duration {
        self.cpu_time
    }

    pub fn is_thread(&self) -> bool {
        self.is_thread
    }

    pub fn has_children(&self) -> bool {
        self.has_children
    }

    pub fn is_collapsed(&self) -> bool {
        self.is_collapsed
    }
}

/// Flatten a `process::Node` tree into an ordered list of `Row`s.
///
/// The connector strings use Unicode box-drawing characters (├─, └─, │)
/// to reproduce an htop-style tree appearance in a columnar table.
pub fn flatten(
    root: &crate::process::Node,
    show_threads: bool,
    collapsed: &HashSet<Pid>,
) -> Vec<Row> {
    let ctx = FlattenCtx {
        root_name: root.name(),
        show_threads,
        collapsed,
    };
    let mut out = Vec::new();
    flatten_node(root, &ctx, "", true, true, &mut out);
    out
}

/// Static context shared across all recursive calls to `flatten_node`.
struct FlattenCtx<'a> {
    root_name: &'a str,
    show_threads: bool,
    collapsed: &'a HashSet<Pid>,
}

/// Choose how a node's command string should appear in the tree.
///
/// Root shows full cmdline; same-binary children strip argv[0] since the
/// binary is implied by tree context; threads show their kernel name.
fn display_command(node: &crate::process::Node, is_root: bool, root_name: &str) -> String {
    if is_root {
        if node.cmdline().is_empty() {
            node.name().to_owned()
        } else {
            node.cmdline().to_owned()
        }
    } else if node.is_thread() {
        node.name().to_owned()
    } else if node.name() == root_name {
        // Same binary as the root — strip argv[0] and show just the arguments.
        match node.cmdline().find(' ') {
            Some(pos) => node.cmdline()[pos + 1..].to_owned(),
            None => node.name().to_owned(),
        }
    } else {
        // Different binary — show the full cmdline.
        if node.cmdline().is_empty() {
            node.name().to_owned()
        } else {
            node.cmdline().to_owned()
        }
    }
}

/// Recursive helper that carries the accumulated indentation prefix.
fn flatten_node(
    node: &crate::process::Node,
    ctx: &FlattenCtx<'_>,
    prefix: &str,
    is_root: bool,
    is_last: bool,
    out: &mut Vec<Row>,
) {
    // Root node gets no connector; subsequent nodes get tree-art glyphs.
    let connector = if is_root {
        String::new()
    } else if is_last {
        format!("{prefix}└─ ")
    } else {
        format!("{prefix}├─ ")
    };

    let cmdline = display_command(node, is_root, ctx.root_name);

    let visible_children: Vec<_> = node
        .children()
        .iter()
        .filter(|c| ctx.show_threads || !c.is_thread())
        .collect();
    let is_collapsed = ctx.collapsed.contains(&node.pid());

    out.push(Row {
        connector,
        pid: node.pid(),
        cmdline,
        user: node.user().to_owned(),
        state: node.state(),
        cpu_pct: node.cpu_pct(),
        mem_pct: node.mem_pct(),
        mem_rss_bytes: node.mem_rss_bytes(),
        elapsed: node.elapsed(),
        cpu_time: node.cpu_time(),
        is_thread: node.is_thread(),
        has_children: !visible_children.is_empty(),
        is_collapsed,
    });

    // Skip children when this node is collapsed.
    if is_collapsed {
        return;
    }

    // The child prefix extends the current prefix by one "column" worth of
    // indentation.  If the current node is not the last sibling we draw a
    // vertical bar; otherwise we draw spaces so the tree closes cleanly.
    let child_prefix = if is_root {
        String::new()
    } else {
        format!("{prefix}{}", if is_last { "   " } else { "│  " })
    };

    let n = visible_children.len();
    for (i, child) in visible_children.iter().enumerate() {
        flatten_node(child, ctx, &child_prefix, false, i == n - 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::Node;

    // Construct a minimal Node for testing. This is in the same crate so the
    // struct literal is accessible (fields are private but same-crate access
    // is permitted via the test module).
    fn make_node(pid: i32, name: &str) -> Node {
        // Node fields are private; use process::make_test_node exposed for tests,
        // or construct via the public builder. Since Node has no public constructor,
        // we call the module-level test helper defined in process.rs.
        crate::process::make_test_node(pid, name)
    }

    #[test]
    fn flatten_single_node() {
        let root = make_node(1, "root");
        let no_collapsed = HashSet::new();
        let rows = flatten(&root, false, &no_collapsed);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].connector(), "");
        assert_eq!(rows[0].pid(), Pid::new(1));
    }

    #[test]
    fn flatten_two_children_connectors() {
        let mut root = make_node(1, "root");
        crate::process::push_child(&mut root, make_node(2, "child1"));
        crate::process::push_child(&mut root, make_node(3, "child2"));
        let no_collapsed = HashSet::new();
        let rows = flatten(&root, false, &no_collapsed);
        assert_eq!(rows.len(), 3);
        // First child is not last → ├─
        assert_eq!(rows[1].connector(), "├─ ");
        // Second child is last → └─
        assert_eq!(rows[2].connector(), "└─ ");
    }

    #[test]
    fn flatten_thread_hidden_by_default() {
        let mut root = make_node(1, "root");
        let thread = crate::process::make_test_thread(10, "thread");
        crate::process::push_child(&mut root, thread);
        let no_collapsed = HashSet::new();
        let rows = flatten(&root, false, &no_collapsed);
        assert_eq!(rows.len(), 1, "thread should be hidden");
    }

    #[test]
    fn flatten_thread_shown_when_requested() {
        let mut root = make_node(1, "root");
        let thread = crate::process::make_test_thread(10, "thread");
        crate::process::push_child(&mut root, thread);
        let no_collapsed = HashSet::new();
        let rows = flatten(&root, true, &no_collapsed);
        assert_eq!(rows.len(), 2, "thread should appear");
        assert!(rows[1].is_thread());
    }

    #[test]
    fn flatten_collapsed_hides_children() {
        let mut root = make_node(1, "root");
        crate::process::push_child(&mut root, make_node(2, "child1"));
        crate::process::push_child(&mut root, make_node(3, "child2"));
        let collapsed = HashSet::from([Pid::new(1)]);
        let rows = flatten(&root, false, &collapsed);
        assert_eq!(
            rows.len(),
            1,
            "children should be hidden when root is collapsed"
        );
        assert!(rows[0].is_collapsed());
    }

    #[test]
    fn flatten_collapsed_subtree() {
        let mut root = make_node(1, "root");
        let mut child = make_node(2, "child");
        crate::process::push_child(&mut child, make_node(3, "grandchild"));
        crate::process::push_child(&mut root, child);
        // Collapse child (pid 2), not root.
        let collapsed = HashSet::from([Pid::new(2)]);
        let rows = flatten(&root, false, &collapsed);
        assert_eq!(rows.len(), 2, "grandchild should be hidden");
        assert!(!rows[0].is_collapsed());
        assert!(rows[1].is_collapsed());
    }

    #[test]
    fn display_command_root_uses_full_cmdline() {
        let mut node = make_node(1, "myapp");
        crate::process::set_cmdline(&mut node, "myapp --foo bar");
        assert_eq!(display_command(&node, true, "myapp"), "myapp --foo bar");
    }

    #[test]
    fn display_command_same_binary_strips_argv0() {
        let mut node = make_node(2, "myapp");
        crate::process::set_cmdline(&mut node, "myapp --child-flag");
        assert_eq!(display_command(&node, false, "myapp"), "--child-flag");
    }

    #[test]
    fn display_command_same_binary_no_args_falls_back_to_name() {
        let mut node = make_node(2, "myapp");
        crate::process::set_cmdline(&mut node, "myapp");
        assert_eq!(display_command(&node, false, "myapp"), "myapp");
    }

    #[test]
    fn display_command_thread_uses_name() {
        let mut node = crate::process::make_test_thread(10, "worker");
        crate::process::set_cmdline(&mut node, "some cmdline");
        assert_eq!(display_command(&node, false, "myapp"), "worker");
    }
}
