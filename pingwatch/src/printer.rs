//! Printer entry point and event loop.
//!
//! This module is the imperative shell of the printer. It owns the tokio
//! event loop that multiplexes incoming `PingEvent`s with the spinner tick,
//! and delegates all mutation of on-screen state to `PrinterState`. It does
//! not know how state is represented or how strings are rendered.

use std::sync::Arc;
#[cfg(any(feature = "animated-spinners", test))]
use std::time::Duration;

use tokio::sync::mpsc;

use crate::{event, spinner_style::SpinnerStyle, types};

mod render;
mod state;

use state::PrinterState;

/// Drive the printer until the incoming event channel is closed.
///
/// The loop is biased towards draining incoming events before firing the next
/// spinner tick so that bursts of events don't get starved by the ticker.
pub async fn run_printer(
    hosts: Arc<[types::Hostname]>,
    spinner_style: SpinnerStyle,
    mut rx: mpsc::Receiver<event::PingEvent>,
) {
    let mut state = PrinterState::new(hosts, spinner_style);

    #[cfg(feature = "animated-spinners")]
    {
        let tick_interval = Duration::from_millis(spinner_style.interval_ms());
        let mut ticker = tokio::time::interval(tick_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                biased;
                maybe_ev = rx.recv() => {
                    let Some(ev) = maybe_ev else { break };
                    state.handle(ev);
                }
                _ = ticker.tick() => state.tick(),
            }
        }
    }

    #[cfg(not(feature = "animated-spinners"))]
    while let Some(ev) = rx.recv().await {
        state.handle(ev);
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;

    fn make_hosts(names: &[&str]) -> Arc<[types::Hostname]> {
        Arc::from(
            names
                .iter()
                .map(|&s| s.parse::<types::Hostname>().unwrap())
                .collect::<Vec<_>>(),
        )
    }

    fn idx(i: usize) -> types::HostIdx {
        types::HostIdx::new(i)
    }

    #[tokio::test]
    async fn exits_when_channel_already_closed() {
        let (tx, rx) = mpsc::channel::<event::PingEvent>(8);
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), SpinnerStyle::default(), rx),
        )
        .await
        .expect("printer should exit immediately when the channel is already closed");
    }

    #[rstest::rstest]
    #[case::resolved(event::PingEvent::Resolved {
        idx: idx(0),
        addr: "127.0.0.1".parse::<IpAddr>().unwrap(),
    })]
    #[case::success(event::PingEvent::Success {
        idx: idx(0),
        rtt: Duration::from_millis(10),
    })]
    #[case::failure(event::PingEvent::Failure {
        idx: idx(0),
        error: "timeout".into(),
    })]
    #[case::resolution_failed(event::PingEvent::ResolutionFailed {
        idx: idx(0),
        error: "nxdomain".into(),
    })]
    #[tokio::test]
    async fn handles_event_variant_without_panic(#[case] ev: event::PingEvent) {
        let (tx, rx) = mpsc::channel(2);
        tx.send(ev).await.unwrap();
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), SpinnerStyle::default(), rx),
        )
        .await
        .expect("printer should handle this event and exit");
    }

    #[tokio::test]
    async fn ignores_out_of_range_host_idx() {
        let (tx, rx) = mpsc::channel(8);
        tx.send(event::PingEvent::Success {
            idx: idx(99),
            rtt: Duration::from_millis(1),
        })
        .await
        .unwrap();
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), SpinnerStyle::default(), rx),
        )
        .await
        .expect("printer should skip out-of-range events without panicking");
    }

    #[tokio::test]
    async fn routes_events_to_correct_bar_for_multiple_hosts() {
        let addr: IpAddr = "10.0.0.1".parse().unwrap();
        let (tx, rx) = mpsc::channel(32);
        for i in 0..3 {
            tx.send(event::PingEvent::Resolved { idx: idx(i), addr })
                .await
                .unwrap();
            tx.send(event::PingEvent::Success {
                idx: idx(i),
                rtt: Duration::from_millis(5 * (i as u64 + 1)),
            })
            .await
            .unwrap();
        }
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(2),
            run_printer(
                make_hosts(&["host-a", "host-b", "host-c"]),
                SpinnerStyle::default(),
                rx,
            ),
        )
        .await
        .expect("printer should handle events across multiple hosts");
    }
}
