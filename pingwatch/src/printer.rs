use crate::{event::PingEvent, types::Hostname};
use chrono::Local;
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Owns all indicatif state and drives every display update from `PingEvent`s.
///
/// Runs as an async task; `rx.recv().await` yields between events so the
/// executor stays free. Returns when the channel is exhausted (all senders
/// have been dropped).
pub async fn run_printer(hosts: Arc<[Hostname]>, mut rx: mpsc::Receiver<PingEvent>) {
    let multi = MultiProgress::new();

    // Column width sized to the longest hostname so the status column aligns.
    let prefix_width = hosts.iter().map(|h| h.as_str().len()).max().unwrap_or(0);

    let style_ok = make_style("green", prefix_width);
    let style_wait = make_style("yellow", prefix_width);

    // Initial state is "resolving" because the first event from every worker
    // is always Resolved or ResolutionFailed.
    let bars: Vec<ProgressBar> = hosts
        .iter()
        .map(|host| {
            let pb = multi.add(ProgressBar::new_spinner());
            pb.set_style(style_wait.clone());
            pb.set_prefix(host.to_string());
            pb.set_message("resolving...");
            pb.enable_steady_tick(Duration::from_millis(80));
            pb
        })
        .collect();

    // Track whether each bar is currently showing the "ok" style to avoid
    // cloning and re-applying the same style on every ping event.
    let mut bar_is_ok = vec![false; bars.len()];

    while let Some(event) = rx.recv().await {
        let idx = event.idx();
        // Defensive: skip events with an out-of-range index rather than panic.
        let Some(bar) = bars.get(idx.as_usize()) else {
            continue;
        };
        match event {
            PingEvent::Resolved { addr, .. } => {
                bar.set_message(format!("resolved → {addr}"));
            }
            PingEvent::ResolutionFailed { error, .. } => {
                bar.finish_with_message(format!("resolution failed: {error}"));
            }
            PingEvent::ClientError { error, .. } => {
                bar.finish_with_message(format!("client error: {error}"));
            }
            PingEvent::Success { rtt, .. } => {
                let ms = rtt.as_secs_f64() * 1000.0;
                if !bar_is_ok[idx.as_usize()] {
                    bar.set_style(style_ok.clone());
                    bar_is_ok[idx.as_usize()] = true;
                }
                bar.set_message(format!("rtt={ms:.1}ms"));
            }
            PingEvent::Failure { error, .. } => {
                // Persistent line: accumulates above the spinner rows in the
                // scroll buffer and is never overwritten.
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
                // Safe: hosts.len() == bars.len(); bounds already checked above.
                let host = &hosts[idx.as_usize()];
                let _ = multi.println(format!(
                    "{timestamp} {host} {}: {error}",
                    style("FAILED").red()
                ));
                if bar_is_ok[idx.as_usize()] {
                    bar.set_style(style_wait.clone());
                    bar_is_ok[idx.as_usize()] = false;
                }
                bar.set_message(style("waiting").yellow().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event::PingEvent,
        types::{HostIdx, Hostname},
    };
    use rstest::rstest;
    use std::{net::IpAddr, sync::Arc};
    use tokio::sync::mpsc;

    fn make_hosts(names: &[&str]) -> Arc<[Hostname]> {
        Arc::from(
            names
                .iter()
                .map(|&s| s.parse::<Hostname>().unwrap())
                .collect::<Vec<_>>(),
        )
    }

    fn idx(i: usize) -> HostIdx {
        HostIdx::new(i)
    }

    // Printer must drain and return as soon as the channel is exhausted.
    #[tokio::test]
    async fn exits_when_channel_already_closed() {
        let (tx, rx) = mpsc::channel::<PingEvent>(8);
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), rx),
        )
        .await
        .expect("printer should exit immediately when the channel is already closed");
    }

    // Each PingEvent variant must be handled in isolation without panicking.
    // Named cases keep the test output readable when a single variant regresses.
    #[rstest]
    #[case::resolved(PingEvent::Resolved {
        idx: idx(0),
        addr: "127.0.0.1".parse::<IpAddr>().unwrap(),
    })]
    #[case::success(PingEvent::Success {
        idx: idx(0),
        rtt: Duration::from_millis(10),
    })]
    #[case::failure(PingEvent::Failure {
        idx: idx(0),
        error: "timeout".into(),
    })]
    #[case::client_error(PingEvent::ClientError {
        idx: idx(0),
        error: "no socket".into(),
    })]
    #[case::resolution_failed(PingEvent::ResolutionFailed {
        idx: idx(0),
        error: "nxdomain".into(),
    })]
    #[tokio::test]
    async fn handles_event_variant_without_panic(#[case] event: PingEvent) {
        let (tx, rx) = mpsc::channel(2);
        tx.send(event).await.unwrap();
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), rx),
        )
        .await
        .expect("printer should handle this event and exit");
    }

    // Out-of-range idx must be silently skipped, not panic.
    #[tokio::test]
    async fn ignores_out_of_range_host_idx() {
        let (tx, rx) = mpsc::channel(8);
        tx.send(PingEvent::Success {
            idx: idx(99), // only one host registered
            rtt: Duration::from_millis(1),
        })
        .await
        .unwrap();
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), rx),
        )
        .await
        .expect("printer should skip out-of-range events without panicking");
    }

    // Multi-host layout: events for different host indices must each route to
    // the correct progress bar without index confusion.
    #[tokio::test]
    async fn routes_events_to_correct_bar_for_multiple_hosts() {
        let addr: IpAddr = "10.0.0.1".parse().unwrap();
        let (tx, rx) = mpsc::channel(32);
        for i in 0..3 {
            tx.send(PingEvent::Resolved { idx: idx(i), addr })
                .await
                .unwrap();
            tx.send(PingEvent::Success {
                idx: idx(i),
                rtt: Duration::from_millis(5 * (i as u64 + 1)),
            })
            .await
            .unwrap();
        }
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(2),
            run_printer(make_hosts(&["host-a", "host-b", "host-c"]), rx),
        )
        .await
        .expect("printer should handle events across multiple hosts");
    }
}

/// Builds a spinner `ProgressStyle` for the given terminal color keyword.
fn make_style(color: &str, prefix_width: usize) -> ProgressStyle {
    ProgressStyle::default_spinner()
        .tick_chars("✶✸✹✺✹✷")
        .template(&format!(
            "{{spinner:.{color}}} {{prefix:<{prefix_width}}} {{msg}}"
        ))
        .expect("valid template")
}
