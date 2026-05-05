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

/// Bytes-per-second read and write rates for a process.
#[derive(Copy, Clone, Debug, Default)]
pub struct IoRate {
    read: u64,
    write: u64,
}

impl IoRate {
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
    /// Bytes/sec read/write since the previous sample (0 on first sample or permission denied).
    io: IoRate,
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

    pub fn io(&self) -> IoRate {
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
pub fn collect_tree(
    root_pid: Pid,
    prev_ticks: &mut HashMap<Pid, u64>,
    prev_io: &mut HashMap<Pid, IoRate>,
    elapsed_secs: f64,
    cfg: &SystemConfig,
    uid_map: &HashMap<u32, String>,
) -> anyhow::Result<Tree> {
    let proc = Process::new(root_pid.get())?;
    let stat = proc.stat()?;

    let current_ticks = stat.utime + stat.stime;
    let delta = current_ticks.saturating_sub(*prev_ticks.get(&root_pid).unwrap_or(&current_ticks));
    let cpu_pct = if elapsed_secs > 0.0 {
        (delta as f64 / cfg.ticks_per_second() as f64) / elapsed_secs * 100.0
    } else {
        0.0
    };
    prev_ticks.insert(root_pid, current_ticks);
    // Total CPU time (user + system) accumulated by this process.
    let cpu_time = Duration::from_secs_f64(current_ticks as f64 / cfg.ticks_per_second() as f64);

    // stat.rss is in pages; convert to bytes then to a percentage of total RAM.
    let mem_rss_bytes = stat.rss * cfg.page_size();
    let mem_pct = if cfg.mem_total_bytes() > 0 {
        mem_rss_bytes as f64 / cfg.mem_total_bytes() as f64 * 100.0
    } else {
        0.0
    };

    // /proc/<pid>/io is only readable by the owning user or root.
    let (io_read_raw, io_write_raw) = match proc.io() {
        Ok(io) => (io.read_bytes, io.write_bytes),
        Err(procfs::ProcError::PermissionDenied(_)) => (0, 0),
        Err(e) => return Err(e.into()),
    };
    let io = if let Some(prev) = prev_io.get(&root_pid) {
        if elapsed_secs > 0.0 {
            IoRate::new(
                (io_read_raw.saturating_sub(prev.read()) as f64 / elapsed_secs) as u64,
                (io_write_raw.saturating_sub(prev.write()) as f64 / elapsed_secs) as u64,
            )
        } else {
            IoRate::default()
        }
    } else {
        IoRate::default()
    };
    prev_io.insert(root_pid, IoRate::new(io_read_raw, io_write_raw));

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
    let elapsed = compute_elapsed(stat.starttime, cfg.ticks_per_second());

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

    let children: Vec<Node> = all_processes()?
        .filter_map(|r| r.ok())
        .filter(|p| p.stat().map(|s| s.ppid == root_pid.get()).unwrap_or(false))
        .filter_map(|p| {
            collect_tree(
                Pid::new(p.pid()),
                prev_ticks,
                prev_io,
                elapsed_secs,
                cfg,
                uid_map,
            )
            .ok()
            .and_then(|t| t.nodes.into_iter().next())
        })
        .collect();

    let thread_nodes: Vec<Node> = proc
        .tasks()
        .map(|tasks| {
            tasks
                .filter_map(|r| r.ok())
                .filter(|t| t.tid != root_pid.get())
                .filter_map(|task| {
                    let tstat = task.stat().ok()?;
                    let thread_ticks = tstat.utime + tstat.stime;
                    let tid = Pid::new(task.tid);
                    let thread_delta =
                        thread_ticks.saturating_sub(*prev_ticks.get(&tid).unwrap_or(&thread_ticks));
                    let thread_cpu = if elapsed_secs > 0.0 {
                        (thread_delta as f64 / cfg.ticks_per_second() as f64) / elapsed_secs * 100.0
                    } else {
                        0.0
                    };
                    prev_ticks.insert(tid, thread_ticks);
                    let thread_cpu_time = Duration::from_secs_f64(
                        thread_ticks as f64 / cfg.ticks_per_second() as f64,
                    );
                    // /proc/<pid>/task/<tid>/comm gives the full thread name
                    // without the 15-char truncation of stat.comm.
                    let thread_name = fs::read_to_string(format!(
                        "/proc/{}/task/{}/comm",
                        root_pid.get(),
                        task.tid
                    ))
                    .map(|s| s.trim_end().to_owned())
                    .unwrap_or_else(|_| tstat.comm.clone());
                    Some(Node {
                        pid: tid,
                        name: thread_name.clone(),
                        cmdline: thread_name,
                        user: user.clone(),
                        state: tstat.state,
                        cpu_pct: Percent::new(thread_cpu),
                        mem_rss_bytes: tstat.rss * cfg.page_size(),
                        mem_pct: Percent::new(0.0),
                        io: IoRate::default(),
                        elapsed: Duration::ZERO,
                        cpu_time: thread_cpu_time,
                        parent_name: stat.comm.clone(),
                        children: Tree::default(),
                        is_thread: true,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Build the tree: root node first, then children and threads.
    Ok(std::iter::once(Node {
        pid: root_pid,
        name: stat.comm,
        cmdline,
        user,
        state: stat.state,
        cpu_pct: Percent::new(cpu_pct),
        mem_rss_bytes,
        mem_pct: Percent::new(mem_pct),
        io,
        elapsed,
        cpu_time,
        parent_name,
        children: Tree::default(),
        is_thread: false,
    })
    .chain(children)
    .chain(thread_nodes)
    .collect())
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
            io: IoRate::default(),
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
}
