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
mod event;
mod printer;
pub mod types;
mod worker;

use std::sync::Arc;
use surge_ping::PingIdentifier;
use tokio::sync::mpsc;

use crate::types::HostIdx;

pub async fn run(args: cli::Args) -> anyhow::Result<()> {
    let hosts: Arc<[types::Hostname]> = Arc::from(args.hosts);
    let interval = args.interval;
    let timeout = args.timeout;

    // Bounded channel: workers back-pressure when the printer lags.
    // At <=10 hosts x 1 ping/s, 64 slots is several seconds of headroom.
    let (tx, rx) = mpsc::channel::<event::PingEvent>(64);

    // Spawn the printer as a regular async task; recv is now non-blocking.
    let printer = tokio::spawn({
        let hosts = Arc::clone(&hosts);
        async move { printer::run_printer(hosts, rx).await }
    });

    // Derive a unique ICMP echo identifier per host from the process ID so
    // concurrent pingers don't respond to each other's replies.
    let base_id = std::process::id() as u16;

    // Spawn one async task per host; each gets its own sender clone.
    let tasks: Vec<_> = hosts
        .iter()
        .enumerate()
        .map(|(i, host)| {
            let id = PingIdentifier(base_id.wrapping_add(i as u16));
            let cfg = worker::WorkerConfig {
                host: host.clone(),
                idx: HostIdx::new(i),
                id,
                interval,
                timeout,
            };
            tokio::spawn(worker::run_worker(cfg, tx.clone()))
        })
        .collect();

    // Drop main's sender so the printer exits once all tasks complete.
    drop(tx);

    // All tasks loop forever; awaiting them keeps main alive until Ctrl-C.
    for task in tasks {
        let _ = task.await;
    }

    // Printer drains any buffered events and then exits naturally.
    let _ = printer.await;

    Ok(())
}
