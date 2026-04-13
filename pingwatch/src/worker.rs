use crate::{
    event::PingEvent,
    types::{HostIdx, Hostname},
};
use std::{net::IpAddr, time::Duration};
use surge_ping::{Client, Config, ICMP, PingIdentifier, PingSequence};
use tokio::sync::mpsc;

/// Per-worker configuration: host identity and timing parameters.
pub struct WorkerConfig {
    pub host: Hostname,
    pub idx: HostIdx,
    pub id: PingIdentifier,
    pub interval: Duration,
    pub timeout: Duration,
}

/// Resolves a host, creates an ICMP client, and pings in a loop -- emitting a
/// typed `PingEvent` for every outcome. Contains zero display logic.
///
/// `tx` is moved in so it drops automatically when the task exits, contributing
/// to the "all senders gone -> printer exits" signal without explicit coordination.
pub async fn run_worker(cfg: WorkerConfig, tx: mpsc::Sender<PingEvent>) {
    let addr = match cfg.host.resolve().await {
        Ok(addr) => {
            let _ = tx.send(PingEvent::Resolved { idx: cfg.idx, addr }).await;
            addr
        }
        Err(e) => {
            let _ = tx
                .send(PingEvent::ResolutionFailed {
                    idx: cfg.idx,
                    error: e.to_string(),
                })
                .await;
            return;
        }
    };

    let icmp_kind = match addr {
        IpAddr::V4(_) => ICMP::V4,
        IpAddr::V6(_) => ICMP::V6,
    };

    let client = match Client::new(&Config::builder().kind(icmp_kind).build()) {
        Ok(c) => c,
        Err(e) => {
            let _ = tx
                .send(PingEvent::ClientError {
                    idx: cfg.idx,
                    error: e.to_string(),
                })
                .await;
            return;
        }
    };

    let mut pinger = client.pinger(addr, cfg.id).await;
    pinger.timeout(cfg.timeout);

    let mut seq: u16 = 0;
    loop {
        match pinger.ping(PingSequence(seq), &[0u8; 8]).await {
            Ok((_, rtt)) => {
                // Break when the printer has exited -- no point continuing.
                if tx
                    .send(PingEvent::Success { idx: cfg.idx, rtt })
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(e) => {
                if tx
                    .send(PingEvent::Failure {
                        idx: cfg.idx,
                        error: e.to_string(),
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
        seq = seq.wrapping_add(1);
        tokio::time::sleep(cfg.interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event::PingEvent,
        types::{HostIdx, Hostname},
    };
    use std::time::Duration;
    use tokio::sync::mpsc;

    fn cfg(host: &str) -> WorkerConfig {
        WorkerConfig {
            host: host.parse::<Hostname>().unwrap(),
            idx: HostIdx::new(0),
            id: PingIdentifier(42),
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
            cfg("this.host.does.not.exist.invalid"),
            tx,
        ));

        let event = rx.recv().await.expect("expected at least one event");
        assert!(
            matches!(event, PingEvent::ResolutionFailed { .. }),
            "expected ResolutionFailed, got {event:?}"
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
        tokio::spawn(run_worker(cfg("127.0.0.1"), tx));

        let first = rx.recv().await.expect("expected Resolved event");
        assert!(
            matches!(first, PingEvent::Resolved { .. }),
            "expected Resolved, got {first:?}"
        );
        // Second event is either Success (CAP_NET_RAW available) or ClientError (not).
        let second = rx.recv().await.expect("expected ping result event");
        assert!(
            matches!(
                second,
                PingEvent::Success { .. } | PingEvent::ClientError { .. } | PingEvent::Failure { .. }
            ),
            "expected Success or ClientError or Failure, got {second:?}"
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
            run_worker(cfg("127.0.0.1"), tx),
        )
        .await
        .expect("worker should exit after detecting closed channel");
    }
}
