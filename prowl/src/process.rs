//! Process data collection — imperative shell layer.
//!
//! This module handles all I/O against the Linux procfs filesystem.
//! It produces plain data types consumed by `app` (coordination) and
//! `ui` (rendering), keeping side-effectful code clearly separated from
//! pure logic.

use std::{
    collections::HashMap,
    fmt, fs,
    time::{Duration, UNIX_EPOCH},
};

use procfs::process::{Process, all_processes};

use crate::format::Percent;

/// Newtype wrapping a Linux process/thread ID.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Pid(i32);

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Pid {
    pub fn new(pid: i32) -> Self {
        Self(pid)
    }

    /// Return the raw `i32` value.
    pub fn get(self) -> i32 {
        self.0
    }
}

/// Cumulative read and write bytes for a process.
#[derive(Copy, Clone, Debug, Default)]
pub struct IoTotals {
    read: u64,
    write: u64,
}

impl IoTotals {
    pub fn new(read: u64, write: u64) -> Self {
        Self { read, write }
    }

    pub fn read(self) -> u64 {
        self.read
    }

    pub fn write(self) -> u64 {
        self.write
    }
}

/// System-level constants needed for CPU/memory percentage calculations.
///
/// Collected once at startup and passed through the sampling call chain
/// so callers don't have to remember unit conversions.
#[derive(Copy, Clone, Debug)]
pub struct SystemConfig {
    ticks_per_second: u64,
    page_size: u64,
    /// Total physical RAM in bytes (already multiplied from kilobytes at construction).
    mem_total_bytes: u64,
}

impl SystemConfig {
    pub fn new(ticks_per_second: u64, page_size: u64, mem_total_bytes: u64) -> Self {
        Self {
            ticks_per_second,
            page_size,
            mem_total_bytes,
        }
    }

    pub fn ticks_per_second(self) -> u64 {
        self.ticks_per_second
    }

    pub fn page_size(self) -> u64 {
        self.page_size
    }

    pub fn mem_total_bytes(self) -> u64 {
        self.mem_total_bytes
    }
}

/// A process tree rooted at a single `Node`.
///
/// Implements `FromIterator<Node>`: the first yielded node becomes the root,
/// all subsequent nodes are appended as its direct children.  This lets
/// callers build a tree by collecting from any iterator (channels, worker
/// tasks, test arrays, etc.).
#[derive(Clone, Default)]
pub struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    /// The root node, if the tree is non-empty.
    pub fn root(&self) -> Option<&Node> {
        self.nodes.first()
    }

    /// Consume the tree and return the root node, if non-empty.
    #[cfg(test)]
    pub fn into_root(self) -> Option<Node> {
        self.nodes.into_iter().next()
    }
}

impl From<Node> for Tree {
    fn from(node: Node) -> Self {
        Self {
            nodes: Vec::from([node]),
        }
    }
}

/// Build a tree: first node = root, remaining nodes = its direct children.
impl std::iter::FromIterator<Node> for Tree {
    fn from_iter<I: IntoIterator<Item = Node>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let Some(mut root) = iter.next() else {
            return Self::default();
        };
        std::iter::Extend::extend(&mut root.children, iter);
        Self::from(root)
    }
}

impl std::iter::Extend<Node> for Tree {
    fn extend<I: IntoIterator<Item = Node>>(&mut self, iter: I) {
        self.nodes.extend(iter);
    }
}

/// Full process/thread node in the tree.
#[derive(Clone)]
pub struct Node {
    pid: Pid,
    name: String,
    cmdline: String,
    user: String,
    state: char,
    cpu_pct: Percent,
    mem_rss_bytes: u64,
    mem_pct: Percent,
    /// Cumulative read/write bytes from `/proc/<pid>/io` (0 on permission denied).
    io: IoTotals,
    elapsed: Duration,
    /// Total CPU time consumed (utime + stime from `/proc/<pid>/stat`).
    cpu_time: Duration,
    parent_name: String,
    children: Tree,
    is_thread: bool,
}

impl Node {
    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn name(&self) -> &str {
        &self.name
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

    pub fn mem_rss_bytes(&self) -> u64 {
        self.mem_rss_bytes
    }

    pub fn mem_pct(&self) -> Percent {
        self.mem_pct
    }

    pub fn io(&self) -> IoTotals {
        self.io
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn cpu_time(&self) -> Duration {
        self.cpu_time
    }

    pub fn parent_name(&self) -> &str {
        &self.parent_name
    }

    pub fn children(&self) -> &[Node] {
        &self.children.nodes
    }

    pub fn is_thread(&self) -> bool {
        self.is_thread
    }

    /// Count all thread nodes contained in this subtree, excluding `self`.
    pub fn thread_count(&self) -> usize {
        self.children
            .nodes
            .iter()
            .map(Node::thread_count_inclusive)
            .sum()
    }

    /// Count all descendant process nodes in this subtree, excluding `self`.
    pub fn subprocess_count(&self) -> usize {
        self.children
            .nodes
            .iter()
            .map(Node::subprocess_count_inclusive)
            .sum()
    }

    fn thread_count_inclusive(&self) -> usize {
        usize::from(self.is_thread)
            + self
                .children
                .nodes
                .iter()
                .map(Node::thread_count_inclusive)
                .sum::<usize>()
    }

    fn subprocess_count_inclusive(&self) -> usize {
        usize::from(!self.is_thread)
            + self
                .children
                .nodes
                .iter()
                .map(Node::subprocess_count_inclusive)
                .sum::<usize>()
    }
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
/// waiting for the next refresh cycle.  The returned `Tree` owns the full
/// hierarchy: root node first, child processes and threads as its children.
pub fn collect_tree(
    root_pid: Pid,
    prev_ticks: &mut HashMap<Pid, u64>,
    elapsed_secs: f64,
    cfg: &SystemConfig,
    uid_map: &HashMap<u32, String>,
) -> anyhow::Result<Tree> {
    let proc = Process::new(root_pid.get())?;
    let stat = proc.stat()?;
    let user = resolve_user(&proc, uid_map);

    Ok(std::iter::once(build_node(
        &proc,
        &stat,
        root_pid,
        &user,
        prev_ticks,
        elapsed_secs,
        cfg,
    )?)
    .chain(collect_child_processes(
        root_pid,
        prev_ticks,
        elapsed_secs,
        cfg,
        uid_map,
    )?)
    .chain(collect_threads(
        &proc,
        root_pid,
        &user,
        &stat,
        prev_ticks,
        elapsed_secs,
        cfg,
    ))
    .collect())
}

/// Assemble a single process `Node` from procfs data and sampled metrics.
fn build_node(
    proc: &Process,
    stat: &procfs::process::Stat,
    pid: Pid,
    user: &str,
    prev_ticks: &mut HashMap<Pid, u64>,
    elapsed_secs: f64,
    cfg: &SystemConfig,
) -> anyhow::Result<Node> {
    let (cpu_pct, cpu_time) = sample_cpu(pid, stat, prev_ticks, elapsed_secs, cfg);
    let (mem_rss_bytes, mem_pct) = compute_memory(stat, cfg);
    Ok(Node {
        pid,
        name: stat.comm.clone(),
        cmdline: read_cmdline(proc, stat),
        user: user.to_owned(),
        state: stat.state,
        cpu_pct,
        mem_rss_bytes,
        mem_pct,
        io: read_io_totals(proc)?,
        elapsed: compute_elapsed(stat.starttime, cfg.ticks_per_second()),
        cpu_time,
        parent_name: lookup_parent_name(stat.ppid),
        children: Tree::default(),
        is_thread: false,
    })
}

/// Compute CPU utilisation since the last sample.
///
/// Returns the per-second CPU% and the total accumulated CPU time (utime +
/// stime).  `prev_ticks` is updated in-place so the next call can compute
/// a fresh delta.
fn sample_cpu(
    pid: Pid,
    stat: &procfs::process::Stat,
    prev_ticks: &mut HashMap<Pid, u64>,
    elapsed_secs: f64,
    cfg: &SystemConfig,
) -> (Percent, Duration) {
    let current_ticks = stat.utime + stat.stime;
    let delta = current_ticks.saturating_sub(*prev_ticks.get(&pid).unwrap_or(&current_ticks));
    let cpu_pct = if elapsed_secs > 0.0 {
        (delta as f64 / cfg.ticks_per_second() as f64) / elapsed_secs * 100.0
    } else {
        0.0
    };
    prev_ticks.insert(pid, current_ticks);
    let cpu_time = Duration::from_secs_f64(current_ticks as f64 / cfg.ticks_per_second() as f64);
    (Percent::new(cpu_pct), cpu_time)
}

/// Derive resident memory in bytes and as a percentage of total RAM.
///
/// `stat.rss` is measured in pages; we multiply by the page size once here
/// so downstream code never needs to know the page size.
fn compute_memory(stat: &procfs::process::Stat, cfg: &SystemConfig) -> (u64, Percent) {
    let mem_rss_bytes = stat.rss * cfg.page_size();
    let mem_pct = if cfg.mem_total_bytes() > 0 {
        mem_rss_bytes as f64 / cfg.mem_total_bytes() as f64 * 100.0
    } else {
        0.0
    };
    (mem_rss_bytes, Percent::new(mem_pct))
}

/// Read cumulative I/O totals from `/proc/<pid>/io`.
///
/// `/proc/<pid>/io` is only readable by the process owner or root;
/// `PermissionDenied` gracefully falls back to zero rather than failing
/// the entire tree collection.
fn read_io_totals(proc: &Process) -> anyhow::Result<IoTotals> {
    match proc.io() {
        Ok(io) => Ok(IoTotals::new(io.read_bytes, io.write_bytes)),
        Err(procfs::ProcError::PermissionDenied(_)) => Ok(IoTotals::default()),
        Err(e) => Err(e.into()),
    }
}

/// Map the process's effective UID to a username via the pre-loaded passwd map.
///
/// Falls back to the numeric UID string if the username is unknown, or to
/// an empty string if `/proc/<pid>/status` is unreadable.
fn resolve_user(proc: &Process, uid_map: &HashMap<u32, String>) -> String {
    proc.status()
        .ok()
        .map(|s| {
            uid_map
                .get(&s.euid)
                .cloned()
                .unwrap_or_else(|| s.euid.to_string())
        })
        .unwrap_or_default()
}

/// Read the full command line (argv joined by spaces).
///
/// Kernel threads and processes whose `/proc/<pid>/cmdline` is empty fall
/// back to the short `comm` name from stat.
fn read_cmdline(proc: &Process, stat: &procfs::process::Stat) -> String {
    proc.cmdline()
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| v.join(" "))
        .unwrap_or_else(|| stat.comm.clone())
}

/// Look up the parent process's short name for display context.
///
/// Returns an empty string if the parent has already exited.
fn lookup_parent_name(ppid: i32) -> String {
    Process::new(ppid)
        .and_then(|p| p.stat())
        .map(|s| s.comm)
        .unwrap_or_default()
}

/// Recursively collect direct child processes of `parent_pid`.
///
/// Enumerates all processes via `/proc` and filters to those whose ppid
/// matches.  Each child is itself collected as a full sub-tree.  Processes
/// that vanish mid-collection are silently skipped.
fn collect_child_processes(
    parent_pid: Pid,
    prev_ticks: &mut HashMap<Pid, u64>,
    elapsed_secs: f64,
    cfg: &SystemConfig,
    uid_map: &HashMap<u32, String>,
) -> anyhow::Result<Vec<Node>> {
    Ok(all_processes()?
        .filter_map(|r| r.ok())
        .filter(|p| {
            p.stat()
                .map(|s| s.ppid == parent_pid.get())
                .unwrap_or(false)
        })
        .filter_map(|p| {
            collect_tree(Pid::new(p.pid()), prev_ticks, elapsed_secs, cfg, uid_map)
                .ok()
                .and_then(|t| t.nodes.into_iter().next())
        })
        .collect())
}

/// Collect the threads (tasks) belonging to a process.
///
/// Each thread is represented as a leaf `Node` with `is_thread = true`.
/// The main thread (tid == pid) is excluded since it is the process itself.
/// Thread names are read from `/proc/<pid>/task/<tid>/comm` which provides
/// the full name without the 15-character truncation of `stat.comm`.
fn collect_threads(
    proc: &Process,
    pid: Pid,
    user: &str,
    stat: &procfs::process::Stat,
    prev_ticks: &mut HashMap<Pid, u64>,
    elapsed_secs: f64,
    cfg: &SystemConfig,
) -> Vec<Node> {
    let Ok(tasks) = proc.tasks() else {
        return Vec::new();
    };
    tasks
        .filter_map(|r| r.ok())
        .filter(|t| t.tid != pid.get())
        .filter_map(|task| {
            let tstat = task.stat().ok()?;
            let tid = Pid::new(task.tid);
            let (cpu_pct, cpu_time) = sample_cpu(tid, &tstat, prev_ticks, elapsed_secs, cfg);
            let thread_name =
                fs::read_to_string(format!("/proc/{}/task/{}/comm", pid.get(), task.tid))
                    .map(|s| s.trim_end().to_owned())
                    .unwrap_or_else(|_| tstat.comm.clone());
            Some(Node {
                pid: tid,
                name: thread_name.clone(),
                cmdline: thread_name,
                user: user.to_owned(),
                state: tstat.state,
                cpu_pct,
                mem_rss_bytes: tstat.rss * cfg.page_size(),
                mem_pct: Percent::new(0.0),
                io: IoTotals::default(),
                elapsed: Duration::ZERO,
                cpu_time,
                parent_name: stat.comm.clone(),
                children: Tree::default(),
                is_thread: true,
            })
        })
        .collect()
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

#[cfg(test)]
pub mod tests {
    use super::*;

    fn push_child(parent: &mut Node, child: Node) {
        parent.children.nodes.push(child);
    }

    /// Build a minimal process `Node` for unit tests.
    pub fn make_test_node(pid: i32, name: &str) -> Node {
        Node {
            pid: Pid::new(pid),
            name: name.to_owned(),
            cmdline: String::new(),
            user: String::new(),
            state: 'S',
            cpu_pct: Percent::new(0.0),
            mem_rss_bytes: 0,
            mem_pct: Percent::new(0.0),
            io: IoTotals::default(),
            elapsed: Duration::ZERO,
            cpu_time: Duration::ZERO,
            parent_name: String::new(),
            children: Tree::default(),
            is_thread: false,
        }
    }

    /// Build a minimal thread `Node` for unit tests.
    pub fn make_test_thread(pid: i32, name: &str) -> Node {
        Node {
            is_thread: true,
            ..make_test_node(pid, name)
        }
    }

    /// Overwrite the `cmdline` field of a `Node`.
    pub fn set_cmdline(node: &mut Node, cmdline: &str) {
        node.cmdline = cmdline.to_owned();
    }

    #[test]
    fn counts_threads_and_subprocesses_recursively() {
        let mut root = make_test_node(1, "root");
        let mut child = make_test_node(2, "child");

        push_child(&mut child, make_test_node(3, "grandchild"));
        push_child(&mut child, make_test_thread(11, "thread-b"));
        push_child(&mut root, child);
        push_child(&mut root, make_test_thread(10, "thread-a"));

        assert_eq!(root.thread_count(), 2);
        assert_eq!(root.subprocess_count(), 2);
    }
}
