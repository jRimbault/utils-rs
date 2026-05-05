//! Background task that periodically reads the process tree and publishes
//! snapshots over a `watch` channel.
//!
//! Owns all sampling state (previous CPU ticks, previous IO totals, timing)
//! so `App` stays limited to pure UI concerns.  Runs procfs I/O on a blocking
//! thread via `spawn_blocking` to avoid stalling the async runtime.

use crate::process::{IoRate, Node, Pid, SystemConfig, collect_tree};
use procfs::Current as _;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::{sync::watch, task, time};

/// Mutable per-iteration state threaded through the sampling loop.
///
/// Bundled into a single struct so it can be moved atomically in and out of
/// `spawn_blocking` without separate take/restore operations per field.
struct SamplingState {
    prev_ticks: HashMap<Pid, u64>,
    prev_io: HashMap<Pid, IoRate>,
    prev_instant: Instant,
}

impl SamplingState {
    fn new() -> Self {
        Self {
            prev_ticks: HashMap::new(),
            prev_io: HashMap::new(),
            prev_instant: Instant::now(),
        }
    }

    fn elapsed_secs(&self) -> f64 {
        self.prev_instant.elapsed().as_secs_f64()
    }
}

pub async fn run(
    root_pid: Pid,
    interval: std::time::Duration,
    uid_map: Arc<HashMap<u32, String>>,
    tx: watch::Sender<Option<Node>>,
) {
    // mem_total_kb from Meminfo is in KiB; multiply once here so collect_tree
    // receives bytes and never needs to know the original unit.
    let mem_total_bytes = procfs::Meminfo::current()
        .map(|m| m.mem_total * 1024)
        .unwrap_or(1);
    let cfg = SystemConfig::new(
        procfs::ticks_per_second(),
        procfs::page_size(),
        mem_total_bytes,
    );

    let mut state = SamplingState::new();
    let mut ticker = time::interval(interval);
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;

        let elapsed_secs = state.elapsed_secs();
        let uid_map = Arc::clone(&uid_map);
        // Move sampling state into the blocking closure; recover it on completion
        // so each iteration has an up-to-date baseline.
        let mut moved_state = SamplingState {
            prev_ticks: std::mem::take(&mut state.prev_ticks),
            prev_io: std::mem::take(&mut state.prev_io),
            prev_instant: state.prev_instant,
        };

        let outcome = task::spawn_blocking(move || {
            let result = collect_tree(
                root_pid,
                &mut moved_state.prev_ticks,
                &mut moved_state.prev_io,
                elapsed_secs,
                &cfg,
                &uid_map,
            );
            (result, moved_state)
        })
        .await;

        match outcome {
            Err(_panic) => break,
            Ok((result, returned_state)) => {
                state.prev_ticks = returned_state.prev_ticks;
                state.prev_io = returned_state.prev_io;
                state.prev_instant = Instant::now();

                match result {
                    Ok(node) => {
                        if tx.send(Some(node)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(None);
                        if !e
                            .downcast_ref::<procfs::ProcError>()
                            .is_some_and(|pe| matches!(pe, procfs::ProcError::NotFound(_)))
                        {
                            eprintln!("prowl: collector error: {e:#}");
                        }
                        break;
                    }
                }
            }
        }
    }
    // Dropping `tx` closes the channel; the UI's `rx.changed()` returns `Err`.
}
