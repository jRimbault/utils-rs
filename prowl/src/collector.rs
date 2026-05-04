//! Background task that periodically reads the process tree and publishes
//! snapshots over a `watch` channel.
//!
//! Owns all sampling state (previous CPU ticks, previous IO totals, timing)
//! so `App` stays limited to pure UI concerns.  Runs procfs I/O on a blocking
//! thread via `spawn_blocking` to avoid stalling the async runtime.

use crate::process::{ProcessNode, collect_tree};
use procfs::Current as _;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::{sync::watch, task, time};

pub async fn run(
    root_pid: i32,
    interval: std::time::Duration,
    uid_map: Arc<HashMap<u32, String>>,
    tx: watch::Sender<Option<ProcessNode>>,
) {
    let ticks_per_second = procfs::ticks_per_second();
    let page_size = procfs::page_size();
    let mut prev_ticks: HashMap<i32, u64> = HashMap::new();
    let mut prev_io: HashMap<i32, (u64, u64)> = HashMap::new();
    let mut prev_instant = Instant::now();
    let mut ticker = time::interval(interval);
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;

        let elapsed_secs = prev_instant.elapsed().as_secs_f64();
        let uid_map = Arc::clone(&uid_map);
        // Move sampling state into the blocking closure; recover it on completion
        // so each iteration has an up-to-date baseline.
        let mut ticks = std::mem::take(&mut prev_ticks);
        let mut io = std::mem::take(&mut prev_io);

        let outcome = task::spawn_blocking(move || {
            let mem_total_kb = procfs::Meminfo::current()
                .map(|m| m.mem_total)
                .unwrap_or(1);
            let result = collect_tree(
                root_pid,
                &mut ticks,
                &mut io,
                elapsed_secs,
                ticks_per_second,
                page_size,
                mem_total_kb,
                &uid_map,
            );
            (result, ticks, io)
        })
        .await;

        match outcome {
            Err(_panic) => break,
            Ok((result, returned_ticks, returned_io)) => {
                prev_ticks = returned_ticks;
                prev_io = returned_io;
                prev_instant = Instant::now();

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
