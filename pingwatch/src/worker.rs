use std::time::Duration;
use tokio::sync::mpsc;

use crate::{client, event, types};

/// Per-worker configuration: host identity, timing, and ICMP clients.
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

    // Keep `client` alive for the whole loop: dropping the last handle to a
    // surge_ping::Client stops its receive task and fails pending pings.
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

    // Two configured names resolving to the same address must not collide. On
    // Linux ping sockets the kernel overwrites the echo identifier, so replies
    // are matched by (address, seq) only; workers sharing one socket would both
    // register seq 0 for 127.0.0.1 and one would fail with IdenticalRequests.
    #[tokio::test]
    async fn workers_pinging_same_address_do_not_collide() {
        let clients = client::PingClients::new().unwrap();
        let (tx, mut rx) = mpsc::channel(64);
        for i in 0..2 {
            let cfg = WorkerConfig {
                idx: types::HostIdx::new(i),
                id: surge_ping::PingIdentifier(42 + i as u16),
                clients: clients.clone(),
                ..worker_cfg("127.0.0.1")
            };
            tokio::spawn(run_worker(cfg, tx.clone()));
        }
        drop(tx);

        // Collect a few rounds of results and require every ping to succeed.
        let mut successes = [0usize; 2];
        while successes.iter().any(|&n| n < 3) {
            let msg = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("timed out waiting for ping results")
                .expect("workers exited early");
            match msg {
                event::PingEvent::Resolved { .. } => {}
                event::PingEvent::Success { idx, .. } => successes[idx.as_usize()] += 1,
                other => panic!("expected only successful pings, got {other:?}"),
            }
        }
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
