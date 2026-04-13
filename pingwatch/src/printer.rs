use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;

use crate::{event, types};

/// Owns all indicatif state and drives every display update from `PingEvent`s.
///
/// Runs as an async task; `rx.recv().await` yields between events so the
/// executor stays free. Returns when the channel is exhausted (all senders
/// have been dropped).
pub async fn run_printer(hosts: Arc<[types::Hostname]>, mut rx: mpsc::Receiver<event::PingEvent>) {
    let multi = indicatif::MultiProgress::new();

    // Column width sized to the longest hostname so the status column aligns.
    let prefix_width = hosts.iter().map(|h| h.as_str().len()).max().unwrap_or(0);

    let style_ok = make_style("green", prefix_width);
    let style_wait = make_style("yellow", prefix_width);

    // Initial state is "resolving" because the first event from every worker
    // is always Resolved or ResolutionFailed.
    let bars: Vec<indicatif::ProgressBar> = hosts
        .iter()
        .map(|host| {
            let pb = multi.add(indicatif::ProgressBar::new_spinner());
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

    while let Some(ev) = rx.recv().await {
        let idx = ev.idx();
        // Defensive: skip events with an out-of-range index rather than panic.
        let Some(bar) = bars.get(idx.as_usize()) else {
            continue;
        };
        match ev {
            event::PingEvent::Resolved { addr, .. } => {
                bar.set_message(format!("resolved → {addr}"));
            }
            event::PingEvent::ResolutionFailed { error, .. } => {
                bar.finish_with_message(format!("resolution failed: {error}"));
            }
            event::PingEvent::Success { rtt, .. } => {
                let ms = rtt.as_secs_f64() * 1000.0;
                if !bar_is_ok[idx.as_usize()] {
                    bar.set_style(style_ok.clone());
                    bar_is_ok[idx.as_usize()] = true;
                }
                bar.set_message(format!("rtt={ms:.1}ms"));
            }
            event::PingEvent::Failure { error, .. } => {
                // Persistent line: accumulates above the spinner rows in the
                // scroll buffer and is never overwritten.
                //
                // Layout: dim timestamp  bold-hostname-column  red-FAILED  error
                // Padding the host to prefix_width aligns it with the spinners
                // above and lets the eye scan a stable hostname column.
                // The timestamp is dimmed so it recedes; hostname and FAILED pop.
                let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                // Safe: hosts.len() == bars.len(); bounds already checked above.
                let host = &hosts[idx.as_usize()];
                // Pad before styling: ANSI escape codes would throw off width math.
                let host_col = format!("{:<prefix_width$}", host.as_str());
                let _ = multi.println(format!(
                    "{}  {}  {}  {error}",
                    console::style(timestamp).dim(),
                    console::style(host_col).bold(),
                    console::style("FAILED").red().bold(),
                ));
                if bar_is_ok[idx.as_usize()] {
                    bar.set_style(style_wait.clone());
                    bar_is_ok[idx.as_usize()] = false;
                }
                bar.set_message(console::style("waiting").yellow().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::IpAddr;

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

    // Printer must drain and return as soon as the channel is exhausted.
    #[tokio::test]
    async fn exits_when_channel_already_closed() {
        let (tx, rx) = mpsc::channel::<event::PingEvent>(8);
        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), run_printer(make_hosts(&["h1"]), rx))
            .await
            .expect("printer should exit immediately when the channel is already closed");
    }

    // Each PingEvent variant must be handled in isolation without panicking.
    // Named cases keep the test output readable when a single variant regresses.
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
        tokio::time::timeout(Duration::from_secs(1), run_printer(make_hosts(&["h1"]), rx))
            .await
            .expect("printer should handle this event and exit");
    }

    // Out-of-range idx must be silently skipped, not panic.
    #[tokio::test]
    async fn ignores_out_of_range_host_idx() {
        let (tx, rx) = mpsc::channel(8);
        tx.send(event::PingEvent::Success {
            idx: idx(99), // only one host registered
            rtt: Duration::from_millis(1),
        })
        .await
        .unwrap();
        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), run_printer(make_hosts(&["h1"]), rx))
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
            run_printer(make_hosts(&["host-a", "host-b", "host-c"]), rx),
        )
        .await
        .expect("printer should handle events across multiple hosts");
    }
}

/// Builds a spinner `ProgressStyle` for the given terminal color keyword.
fn make_style(color: &str, prefix_width: usize) -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::default_spinner()
        .tick_chars("✶✸✹✺✹✷")
        .template(&format!(
            "{{spinner:.{color}}} {{prefix:<{prefix_width}}} {{msg}}"
        ))
        .expect("valid template")
}
