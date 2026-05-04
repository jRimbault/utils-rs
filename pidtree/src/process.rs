//! Process data collection — imperative shell layer.
//!
//! This module handles all I/O against the Linux procfs filesystem.
//! It produces plain data types consumed by `app` (coordination) and
//! `ui` (rendering), keeping side-effectful code clearly separated from
//! pure logic.

use std::{
    collections::HashMap,
    fs,
    time::{Duration, UNIX_EPOCH},
};

use procfs::process::{all_processes, Process};

/// Full process/thread node in the tree.
#[derive(Clone)]
pub struct ProcessNode {
    pub pid: i32,
    pub name: String,
    pub cmdline: String,
    pub user: String,
    pub state: char,
    pub cpu_pct: f64,
    pub mem_rss_bytes: u64,
    pub mem_pct: f64,
    /// Bytes/sec read since the previous sample (0 on first sample or permission denied).
    pub io_read_rate: u64,
    /// Bytes/sec written since the previous sample.
    pub io_write_rate: u64,
    pub elapsed: Duration,
    pub parent_name: String,
    pub children: Vec<ProcessNode>,
    pub is_thread: bool,
}

/// Flattened row used by the tree-table widget.
///
/// `connector` contains the full Unicode-art prefix produced by `flatten`,
/// e.g. `"│  ├─ "`.
#[derive(Clone)]
pub struct FlatRow {
    pub pid: i32,
    /// Full tree-art prefix + connector glyph, ready to prepend to `cmdline`.
    pub connector: String,
    /// Full command line (argv joined by spaces); falls back to `stat.comm` for threads
    /// and kernel workers where `/proc/<pid>/cmdline` is empty.
    pub cmdline: String,
    pub user: String,
    pub state: char,
    pub cpu_pct: f64,
    pub mem_pct: f64,
    pub mem_rss_bytes: u64,
    pub is_thread: bool,
}

/// Parse `/etc/passwd` into a `uid → username` map.
///
/// Silently skips malformed lines and returns an empty map on IO error,
/// so callers can degrade to numeric UIDs rather than crash.
pub fn load_uid_map() -> HashMap<u32, String> {
    let content = match fs::read_to_string("/etc/passwd") {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    content
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, ':');
            let name = fields.next()?.to_owned();
            fields.next()?; // password placeholder
            let uid: u32 = fields.next()?.parse().ok()?;
            Some((uid, name))
        })
        .collect()
}

/// Collect the process tree rooted at `root_pid`.
///
/// Threads are always collected so the UI can toggle their visibility without
/// waiting for the next refresh cycle.
///
/// CPU% is computed as the fraction of one second consumed by this process
/// since the last sample.  `prev_ticks` is updated in-place so the next
/// call can compute a fresh delta.
///
/// `/proc/<pid>/io` is readable only by the process owner or root; on
/// `PermissionDenied` the IO fields fall back to 0.  All other errors
/// from procfs are propagated.
#[allow(clippy::too_many_arguments)]
pub fn collect_tree(
    root_pid: i32,
    prev_ticks: &mut HashMap<i32, u64>,
    prev_io: &mut HashMap<i32, (u64, u64)>,
    elapsed_secs: f64,
    ticks_per_second: u64,
    page_size: u64,
    mem_total_kb: u64,
    uid_map: &HashMap<u32, String>,
) -> anyhow::Result<ProcessNode> {
    let proc = Process::new(root_pid)?;
    let stat = proc.stat()?;

    let current_ticks = stat.utime + stat.stime;
    let delta = current_ticks
        .saturating_sub(*prev_ticks.get(&root_pid).unwrap_or(&current_ticks));
    let cpu_pct = if elapsed_secs > 0.0 {
        (delta as f64 / ticks_per_second as f64) / elapsed_secs * 100.0
    } else {
        0.0
    };
    prev_ticks.insert(root_pid, current_ticks);

    // stat.rss is in pages; convert to bytes then to a percentage of total RAM.
    let mem_rss_bytes = stat.rss * page_size;
    let mem_total_bytes = mem_total_kb * 1024;
    let mem_pct = if mem_total_bytes > 0 {
        mem_rss_bytes as f64 / mem_total_bytes as f64 * 100.0
    } else {
        0.0
    };

    // /proc/<pid>/io is only readable by the owning user or root.
    let (io_read_raw, io_write_raw) = match proc.io() {
        Ok(io) => (io.read_bytes, io.write_bytes),
        Err(procfs::ProcError::PermissionDenied(_)) => (0, 0),
        Err(e) => return Err(e.into()),
    };
    let (io_read_rate, io_write_rate) =
        if let Some(&(prev_read, prev_write)) = prev_io.get(&root_pid) {
            if elapsed_secs > 0.0 {
                (
                    (io_read_raw.saturating_sub(prev_read) as f64 / elapsed_secs) as u64,
                    (io_write_raw.saturating_sub(prev_write) as f64 / elapsed_secs) as u64,
                )
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        };
    prev_io.insert(root_pid, (io_read_raw, io_write_raw));

    let user = proc
        .status()
        .ok()
        .map(|s| {
            uid_map
                .get(&s.euid)
                .cloned()
                .unwrap_or_else(|| s.euid.to_string())
        })
        .unwrap_or_default();

    // boot_time() uses chrono::Local in procfs 0.18.
    let elapsed = compute_elapsed(stat.starttime, ticks_per_second);

    let parent_name = Process::new(stat.ppid)
        .and_then(|p| p.stat())
        .map(|s| s.comm)
        .unwrap_or_default();

    let cmdline = proc
        .cmdline()
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| v.join(" "))
        .unwrap_or_else(|| stat.comm.clone());

    let children: Vec<ProcessNode> = all_processes()?
        .filter_map(|r| r.ok())
        .filter(|p| p.stat().map(|s| s.ppid == root_pid).unwrap_or(false))
        .filter_map(|p| {
            collect_tree(
                p.pid(),
                prev_ticks,
                prev_io,
                elapsed_secs,
                ticks_per_second,
                page_size,
                mem_total_kb,
                uid_map,
            )
            .ok()
        })
        .collect();

    let thread_nodes: Vec<ProcessNode> = proc
        .tasks()
        .map(|tasks| {
            tasks
                .filter_map(|r| r.ok())
                .filter(|t| t.tid != root_pid)
                .filter_map(|task| {
                    let tstat = task.stat().ok()?;
                    let thread_ticks = tstat.utime + tstat.stime;
                    let thread_delta = thread_ticks
                        .saturating_sub(*prev_ticks.get(&task.tid).unwrap_or(&thread_ticks));
                    let thread_cpu = if elapsed_secs > 0.0 {
                        (thread_delta as f64 / ticks_per_second as f64) / elapsed_secs * 100.0
                    } else {
                        0.0
                    };
                    prev_ticks.insert(task.tid, thread_ticks);
                    Some(ProcessNode {
                        pid: task.tid,
                        name: tstat.comm.clone(),
                        // Threads share the parent address space; show the parent's
                        // full command with the thread's kernel-visible name in brackets.
                        cmdline: format!("{cmdline} [{}]", tstat.comm),
                        user: user.clone(),
                        state: tstat.state,
                        cpu_pct: thread_cpu,
                        mem_rss_bytes: tstat.rss * page_size,
                        mem_pct: 0.0,
                        io_read_rate: 0,
                        io_write_rate: 0,
                        elapsed: Duration::ZERO,
                        parent_name: stat.comm.clone(),
                        children: Vec::new(),
                        is_thread: true,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Merge threads into children so the tree structure is uniform.
    let mut all_children = children;
    all_children.extend(thread_nodes);

    Ok(ProcessNode {
        pid: root_pid,
        name: stat.comm,
        cmdline,
        user,
        state: stat.state,
        cpu_pct,
        mem_rss_bytes,
        mem_pct,
        io_read_rate,
        io_write_rate,
        elapsed,
        parent_name,
        children: all_children,
        is_thread: false,
    })
}

/// Compute how long the process has been running.
///
/// `starttime` is clock ticks since boot (from `/proc/<pid>/stat`).
fn compute_elapsed(starttime: u64, ticks_per_second: u64) -> Duration {
    // boot_time_secs() avoids the chrono dependency here and returns u64 directly.
    let boot_secs = procfs::boot_time_secs().unwrap_or(0);
    let start_secs = boot_secs.saturating_add(starttime / ticks_per_second.max(1));
    let now_secs = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(start_secs);
    Duration::from_secs(now_secs.saturating_sub(start_secs))
}

/// Flatten a `ProcessNode` tree into an ordered list of `FlatRow`s.
///
/// The connector strings use Unicode box-drawing characters (├─, └─, │)
/// to reproduce an htop-style tree appearance in a columnar table.
pub fn flatten(root: &ProcessNode, show_threads: bool) -> Vec<FlatRow> {
    let mut out = Vec::new();
    flatten_node(root, "", true, true, show_threads, &mut out);
    out
}

/// Recursive helper that carries the accumulated indentation prefix.
fn flatten_node(
    node: &ProcessNode,
    prefix: &str,
    is_root: bool,
    is_last: bool,
    show_threads: bool,
    out: &mut Vec<FlatRow>,
) {
    // Root node gets no connector; subsequent nodes get tree-art glyphs.
    let connector = if is_root {
        String::new()
    } else if is_last {
        format!("{prefix}└─ ")
    } else {
        format!("{prefix}├─ ")
    };

    // Threads have no cmdline; use name as display fallback.
    let cmdline = if node.cmdline.is_empty() {
        node.name.clone()
    } else {
        node.cmdline.clone()
    };

    out.push(FlatRow {
        connector,
        pid: node.pid,
        cmdline,
        user: node.user.clone(),
        state: node.state,
        cpu_pct: node.cpu_pct,
        mem_pct: node.mem_pct,
        mem_rss_bytes: node.mem_rss_bytes,
        is_thread: node.is_thread,
    });

    // The child prefix extends the current prefix by one "column" worth of
    // indentation.  If the current node is not the last sibling we draw a
    // vertical bar; otherwise we draw spaces so the tree closes cleanly.
    let child_prefix = if is_root {
        String::new()
    } else {
        format!("{prefix}{}", if is_last { "   " } else { "│  " })
    };

    let visible: Vec<_> = node
        .children
        .iter()
        .filter(|c| show_threads || !c.is_thread)
        .collect();
    let n = visible.len();
    for (i, child) in visible.iter().enumerate() {
        flatten_node(child, &child_prefix, false, i == n - 1, show_threads, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(pid: i32, name: &str) -> ProcessNode {
        ProcessNode {
            pid,
            name: name.to_owned(),
            cmdline: String::new(),
            user: String::new(),
            state: 'S',
            cpu_pct: 0.0,
            mem_rss_bytes: 0,
            mem_pct: 0.0,
            io_read_rate: 0,
            io_write_rate: 0,
            elapsed: Duration::ZERO,
            parent_name: String::new(),
            children: Vec::new(),
            is_thread: false,
        }
    }

    #[test]
    fn flatten_single_node() {
        let root = make_node(1, "root");
        let rows = flatten(&root, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].connector, "");
        assert_eq!(rows[0].pid, 1);
    }

    #[test]
    fn flatten_two_children_connectors() {
        let mut root = make_node(1, "root");
        root.children.push(make_node(2, "child1"));
        root.children.push(make_node(3, "child2"));
        let rows = flatten(&root, false);
        assert_eq!(rows.len(), 3);
        // First child is not last → ├─
        assert_eq!(rows[1].connector, "├─ ");
        // Second child is last → └─
        assert_eq!(rows[2].connector, "└─ ");
    }

    #[test]
    fn flatten_thread_hidden_by_default() {
        let mut root = make_node(1, "root");
        let mut thread = make_node(10, "thread");
        thread.is_thread = true;
        root.children.push(thread);
        let rows = flatten(&root, false);
        assert_eq!(rows.len(), 1, "thread should be hidden");
    }

    #[test]
    fn flatten_thread_shown_when_requested() {
        let mut root = make_node(1, "root");
        let mut thread = make_node(10, "thread");
        thread.is_thread = true;
        root.children.push(thread);
        let rows = flatten(&root, true);
        assert_eq!(rows.len(), 2, "thread should appear");
        assert!(rows[1].is_thread);
    }
}
