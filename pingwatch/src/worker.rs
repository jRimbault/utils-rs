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
