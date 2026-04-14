//! Pings one or more hosts simultaneously, showing live per-host status via
//! an indicatif MultiProgress TUI.
//!
//! # Architecture -- functional core / imperative shell
//!
//! ```text
//!   +-------------------------------------------------------------+
//!   |  async worker tasks (one per host)                          |
//!   |  - DNS resolution + ICMP ping loop                          |
//!   |  - zero display code; emit typed PingEvents                 |
//!   +----------------------------+--------------------------------+
//!                                |  tokio::sync::mpsc::Sender<PingEvent>
//!                                v  (cloned into each task; bounded, async send)
//!   +-------------------------------------------------------------+
//!   |  async printer task                                         |
//!   |  - owns MultiProgress + all ProgressBars + styles           |
//!   |  - yields between events via Receiver::recv().await         |
//!   |  - exits when all senders drop (channel exhausted)          |
//!   +-------------------------------------------------------------+
//! ```
//!
//! `tokio::sync::mpsc::channel` (bounded) is used so that workers apply
//! backpressure when the printer falls behind. Both sides are async: workers
//! `.await` the send and the printer `.await` each recv.

pub mod cli;
mod client;
mod event;
mod printer;
pub mod spinner_style;
pub mod types;
mod worker;

use std::sync::Arc;
use tokio::sync::mpsc;

/// Install a Ctrl+C handler that restores the terminal cursor.
///
/// Prevents an invisible cursor when the process is interrupted while
/// indicatif's spinner has hidden it.
pub fn setup_ctrlc_handler() -> anyhow::Result<()> {
    ctrlc::set_handler(|| {
        let _ = console::Term::stdout().show_cursor();
        std::process::exit(0);
    })?;
    Ok(())
}

pub async fn run(args: cli::Args) -> anyhow::Result<()> {
    let hosts: Arc<[types::Hostname]> = Arc::from(args.hosts);
    let interval = args.interval;
    let spinner_style = args.spinner_style;
    let timeout = args.timeout;

    // Bounded channel: workers back-pressure when the printer lags.
    // At <=10 hosts x 1 ping/s, 64 slots is several seconds of headroom.
    let (tx, rx) = mpsc::channel::<event::PingEvent>(64);

    let printer = tokio::spawn({
        let hosts = Arc::clone(&hosts);
        async move { printer::run_printer(hosts, spinner_style, rx).await }
    });

    // One ICMP client per protocol version, shared across all workers.
    // Each Client opens a single raw socket; sharing avoids N duplicate sockets.
    let clients = client::PingClients::new()?;

    // Derive a unique ICMP echo identifier per host from the process ID so
    // concurrent pingers don't respond to each other's replies.
    let base_id = std::process::id() as u16;

    let tasks: Vec<_> = hosts
        .iter()
        .enumerate()
        .map(|(i, host)| {
            let cfg = worker::WorkerConfig {
                host: host.clone(),
                idx: types::HostIdx::new(i),
                id: surge_ping::PingIdentifier(base_id.wrapping_add(i as u16)),
                clients: clients.clone(),
                interval,
                timeout,
            };
            tokio::spawn(worker::run_worker(cfg, tx.clone()))
        })
        .collect();

    // Drop main's sender so the printer exits once all tasks complete.
    drop(tx);

    // Awaiting keeps main alive; tasks run until the process is terminated.
    for task in tasks {
        let _ = task.await;
    }

    let _ = printer.await;

    Ok(())
}
