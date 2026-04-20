use std::time::Duration;
use tokio::sync::mpsc;

use crate::{client, event, types};

/// Per-worker configuration: host identity, timing, and shared ICMP clients.
pub struct WorkerConfig {
    pub host: types::Hostname,
    pub idx: types::HostIdx,
    pub id: surge_ping::PingIdentifier,
    pub clients: client::PingClients,
    pub interval: Duration,
    pub timeout: Duration,
}

/// Resolves a host and pings in a loop -- emitting a typed `PingEvent` for
/// every outcome. Contains zero display logic.
///
/// `tx` is moved in so it drops automatically when the task exits, contributing
/// to the "all senders gone -> printer exits" signal without explicit coordination.
pub async fn run_worker(cfg: WorkerConfig, tx: mpsc::Sender<event::PingEvent>) {
    let addr = match cfg.host.resolve().await {
        Ok(addr) => {
            let _ = tx
                .send(event::PingEvent::Resolved { idx: cfg.idx, addr })
                .await;
            addr
        }
        Err(e) => {
            let _ = tx
                .send(event::PingEvent::ResolutionFailed {
                    idx: cfg.idx,
                    error: e,
                })
                .await;
            return;
        }
    };

    let client = cfg.clients.for_addr(addr);
    let mut pinger = client.pinger(addr, cfg.id).await;
    pinger.timeout(cfg.timeout);

    // Use a fixed-interval ticker instead of post-ping sleep so that RTT and
    // processing time don't accumulate as drift. Delay behavior skips missed
    // ticks (e.g. when a ping exceeds the interval) rather than bursting.
    let mut ticker = tokio::time::interval(cfg.interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut seq: u16 = 0;
    loop {
        ticker.tick().await; // first tick fires immediately; subsequent ticks are interval-aligned
        match pinger.ping(surge_ping::PingSequence(seq), &[0u8; 8]).await {
            Ok((_, rtt)) => {
                // Break when the printer has exited -- no point continuing.
                if tx
                    .send(event::PingEvent::Success { idx: cfg.idx, rtt })
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(e) => {
                if tx
                    .send(event::PingEvent::Failure {
                        idx: cfg.idx,
                        error: e.into(),
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
        seq = seq.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_cfg(host: &str) -> WorkerConfig {
        WorkerConfig {
            host: host.parse::<types::Hostname>().unwrap(),
            idx: types::HostIdx::new(0),
            id: surge_ping::PingIdentifier(42),
            clients: client::PingClients::new().unwrap(),
            interval: Duration::from_millis(100),
            timeout: Duration::from_millis(200),
        }
    }

    // Resolution failure: worker emits exactly one ResolutionFailed event, then exits
    // (the sender drop closes the channel, confirming the task has returned).
    #[tokio::test]
    async fn invalid_host_emits_resolution_failed_and_exits() {
        let (tx, mut rx) = mpsc::channel(8);
        tokio::spawn(run_worker(
            worker_cfg("this.host.does.not.exist.invalid"),
            tx,
        ));

        let msg = rx.recv().await.expect("expected at least one event");
        assert!(
            matches!(msg, event::PingEvent::ResolutionFailed { .. }),
            "expected ResolutionFailed, got {msg:?}"
        );
        // Channel must drain: worker returned, all senders dropped.
        assert!(
            rx.recv().await.is_none(),
            "worker should have exited after resolution failure"
        );
    }

    // Happy DNS path (IP literal, no actual DNS round-trip): first event is Resolved.
    #[tokio::test]
    async fn valid_ip_emits_resolved_then_ping_result() {
        let (tx, mut rx) = mpsc::channel(16);
        tokio::spawn(run_worker(worker_cfg("127.0.0.1"), tx));

        let first = rx.recv().await.expect("expected Resolved event");
        assert!(
            matches!(first, event::PingEvent::Resolved { .. }),
            "expected Resolved, got {first:?}"
        );
        // Second event is either Success (CAP_NET_RAW available) or Failure (not).
        let second = rx.recv().await.expect("expected ping result event");
        assert!(
            matches!(
                second,
                event::PingEvent::Success { .. } | event::PingEvent::Failure { .. }
            ),
            "expected Success or Failure, got {second:?}"
        );
    }

    // Backpressure / cancellation: when the receiver is dropped the worker must
    // detect the closed channel and exit rather than spin indefinitely.
    #[tokio::test]
    async fn worker_exits_when_receiver_dropped() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);

        // Worst case: one ping timeout (200 ms) before the worker notices the channel is gone.
        tokio::time::timeout(
            Duration::from_secs(5),
            run_worker(worker_cfg("127.0.0.1"), tx),
        )
        .await
        .expect("worker should exit after detecting closed channel");
    }
}
