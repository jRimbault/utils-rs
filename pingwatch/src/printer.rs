use std::{net::IpAddr, sync::Arc, time::Duration};
use tokio::sync::mpsc;

use crate::{event, spinner_style::SpinnerStyle, types};

/// Owns all indicatif state and drives every display update from `PingEvent`s.
///
/// Runs as an async task; `rx.recv().await` yields between events so the
/// executor stays free. Returns when the channel is exhausted (all senders
/// have been dropped).
pub async fn run_printer(
    hosts: Arc<[types::Hostname]>,
    spinner_style: SpinnerStyle,
    mut rx: mpsc::Receiver<event::PingEvent>,
) {
    let multi = indicatif::MultiProgress::new();

    // Column width sized to the longest hostname so the status column aligns.
    let host_width = hosts.iter().map(|h| h.as_str().len()).max().unwrap_or(0);
    let initial_resolved_width = 0;

    let style_ok = make_style("green", spinner_style);
    let style_wait = make_style("yellow", spinner_style);

    // Initial state is "resolving" because the first event from every worker
    // is always Resolved or ResolutionFailed.
    let bars: Vec<indicatif::ProgressBar> = hosts
        .iter()
        .map(|host| {
            let pb = multi.add(indicatif::ProgressBar::new_spinner());
            pb.set_style(style_wait.clone());
            pb.set_prefix(render_prefix(
                host,
                host_width,
                initial_resolved_width,
                None,
            ));
            pb.set_message("resolving...");
            pb
        })
        .collect();

    // Drive spinner frames from the tokio event loop rather than calling
    // enable_steady_tick(), which spawns one OS thread per ProgressBar.
    // A single interval here replaces N background threads with zero threads.
    let tick_interval = Duration::from_millis(spinner_style.interval_ms());
    let mut ticker = tokio::time::interval(tick_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Track whether each bar is currently showing the "ok" style to avoid
    // cloning and re-applying the same style on every ping event.
    let mut bar_is_ok = vec![false; bars.len()];
    let mut resolved_addrs = vec![None; bars.len()];
    let mut resolved_width = resolved_text_width(&resolved_addrs);

    loop {
        // Prioritise events over ticks so that state updates are never delayed
        // by a tick branch that happens to be ready at the same instant.
        tokio::select! {
            biased;
            maybe_ev = rx.recv() => {
                let Some(ev) = maybe_ev else { break };
                let idx = ev.idx();
                let i = idx.as_usize();
                // Defensive: skip events with an out-of-range index rather than panic.
                let Some(bar) = bars.get(i) else { continue };
                match ev {
                    event::PingEvent::Resolved { addr, .. } => {
                        let display_addr = resolved_addr_for_display(&hosts[i], addr);
                        resolved_addrs[i] = display_addr;
                        let next_resolved_width = resolved_text_width(&resolved_addrs);
                        if next_resolved_width != resolved_width {
                            resolved_width = next_resolved_width;
                            refresh_prefixes(&bars, &hosts, host_width, resolved_width, &resolved_addrs);
                        } else {
                            bar.set_prefix(render_prefix(
                                &hosts[i],
                                host_width,
                                resolved_width,
                                display_addr,
                            ));
                        }
                        bar.set_message("resolved");
                    }
                    event::PingEvent::ResolutionFailed { error, .. } => {
                        bar.finish_with_message(format!("resolution failed: {error}"));
                    }
                    event::PingEvent::Success { rtt, .. } => {
                        let ms = rtt.as_secs_f64() * 1000.0;
                        if !bar_is_ok[i] {
                            bar.set_style(style_ok.clone());
                            bar_is_ok[i] = true;
                        }
                        bar.set_message(format!(
                            "{}{ms:.1}ms",
                            console::style("rtt=").dim().italic(),
                        ));
                    }
                    event::PingEvent::Failure { error, .. } => {
                        // Persistent line: accumulates above the spinner rows in the
                        // scroll buffer and is never overwritten.
                        //
                        // Layout: dim timestamp  bold host/address prefix  red-FAILED  error.
                        // The prefix uses the same padded host column as the spinners,
                        // and includes the resolved address when it adds information.
                        // The timestamp is dimmed so it recedes; prefix and FAILED pop.
                        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                        // Safe: hosts.len() == bars.len(); bounds already checked above.
                        let prefix = render_failure_prefix(
                            &hosts[i],
                            host_width,
                            resolved_width,
                            resolved_addrs[i],
                        );
                        let _ = multi.println(format!(
                            "{}  {}  {}  {error}",
                            console::style(timestamp).dim(),
                            prefix,
                            console::style("FAILED").red().bold(),
                        ));
                        if bar_is_ok[i] {
                            bar.set_style(style_wait.clone());
                            bar_is_ok[i] = false;
                        }
                        bar.set_message(console::style("waiting").yellow().to_string());
                    }
                }
            }
            _ = ticker.tick() => {
                for bar in &bars {
                    bar.tick();
                }
            }
        }
    }
}

/// Builds a spinner `ProgressStyle` for the given terminal color keyword.
fn make_style(color: &str, spinner_style: SpinnerStyle) -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::default_spinner()
        .tick_strings(spinner_style.frames())
        .template(&format!("{{spinner:.{color}}} {{prefix}} {{msg}}"))
        .expect("valid template")
}

fn resolved_addr_for_display(host: &types::Hostname, addr: IpAddr) -> Option<IpAddr> {
    match host.as_str().parse::<IpAddr>() {
        Ok(literal_addr) if literal_addr == addr => None,
        _ => Some(addr),
    }
}

fn refresh_prefixes(
    bars: &[indicatif::ProgressBar],
    hosts: &[types::Hostname],
    host_width: usize,
    resolved_width: usize,
    resolved_addrs: &[Option<IpAddr>],
) {
    for ((bar, host), &resolved_addr) in bars.iter().zip(hosts.iter()).zip(resolved_addrs.iter()) {
        bar.set_prefix(render_prefix(
            host,
            host_width,
            resolved_width,
            resolved_addr,
        ));
    }
}

fn render_prefix(
    host: &types::Hostname,
    host_width: usize,
    resolved_width: usize,
    resolved_addr: Option<IpAddr>,
) -> String {
    format!(
        "{}{}",
        render_host_text(host, host_width),
        render_resolved_text(resolved_width, resolved_addr)
            .map(|text| console::style(text).dim().to_string())
            .unwrap_or_else(|| " ".repeat(resolved_width))
    )
}

fn render_failure_prefix(
    host: &types::Hostname,
    host_width: usize,
    resolved_width: usize,
    resolved_addr: Option<IpAddr>,
) -> String {
    format!(
        "{}{}",
        console::style(render_host_text(host, host_width)).bold(),
        render_resolved_text(resolved_width, resolved_addr)
            .map(|text| console::style(text).dim().to_string())
            .unwrap_or_else(|| " ".repeat(resolved_width))
    )
}

fn render_host_text(host: &types::Hostname, host_width: usize) -> String {
    format!("{:<host_width$}", host.as_str())
}

fn resolved_text_width(resolved_addrs: &[Option<IpAddr>]) -> usize {
    resolved_addrs
        .iter()
        .flatten()
        .map(|addr| format!(" ({addr})").len())
        .max()
        .unwrap_or(0)
}

fn render_resolved_text(resolved_width: usize, resolved_addr: Option<IpAddr>) -> Option<String> {
    if resolved_width == 0 {
        return None;
    }
    Some(format!(
        "{:<resolved_width$}",
        resolved_addr
            .map(|addr| format!(" ({addr})"))
            .unwrap_or_default()
    ))
}

#[cfg(test)]
mod tests {
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

    // Printer must drain and return as soon as the channel is exhausted.
    #[tokio::test]
    async fn exits_when_channel_already_closed() {
        let (tx, rx) = mpsc::channel::<event::PingEvent>(8);
        drop(tx);
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), SpinnerStyle::Dots, rx),
        )
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
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), SpinnerStyle::Dots, rx),
        )
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
        tokio::time::timeout(
            Duration::from_secs(1),
            run_printer(make_hosts(&["h1"]), SpinnerStyle::Dots, rx),
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
                SpinnerStyle::Dots,
                rx,
            ),
        )
        .await
        .expect("printer should handle events across multiple hosts");
    }

    #[test]
    fn hides_resolved_addr_for_ip_literal_inputs() {
        let host = "127.0.0.1".parse::<types::Hostname>().unwrap();
        let addr: IpAddr = "127.0.0.1".parse().unwrap();

        assert_eq!(resolved_addr_for_display(&host, addr), None);
        assert_eq!(render_host_text(&host, host.as_str().len()), "127.0.0.1");
        assert_eq!(render_resolved_text(0, None), None);
    }

    #[test]
    fn shows_resolved_addr_for_hostname_inputs() {
        let host = "example.com".parse::<types::Hostname>().unwrap();
        let addr: IpAddr = "93.184.216.34".parse().unwrap();
        let resolved_width = resolved_text_width(&[Some(addr)]);

        assert_eq!(resolved_addr_for_display(&host, addr), Some(addr));
        assert_eq!(render_host_text(&host, host.as_str().len()), "example.com");
        assert_eq!(
            render_resolved_text(resolved_width, Some(addr)),
            Some(format!(
                "{:<width$}",
                format!(" ({addr})"),
                width = resolved_width
            ))
        );
    }

    #[test]
    fn keeps_prefix_width_stable_with_optional_resolved_addr() {
        let host = "example.com".parse::<types::Hostname>().unwrap();
        let literal = "127.0.0.1".parse::<types::Hostname>().unwrap();
        let addr: IpAddr = "93.184.216.34".parse().unwrap();
        let resolved_width = resolved_text_width(&[Some(addr)]);

        let with_addr = format!(
            "{}{}",
            render_host_text(&host, 11),
            render_resolved_text(resolved_width, Some(addr)).unwrap()
        );
        let without_addr = format!(
            "{}{}",
            render_host_text(&literal, 11),
            render_resolved_text(resolved_width, None).unwrap()
        );

        assert_eq!(with_addr.len(), without_addr.len());
        assert!(with_addr.contains(&format!("({addr})")));
        assert!(!without_addr.contains('('));
    }

    #[test]
    fn resolved_width_tracks_longest_seen_addr() {
        let ipv4: IpAddr = "93.184.216.34".parse().unwrap();
        let ipv6: IpAddr = "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff".parse().unwrap();

        assert_eq!(resolved_text_width(&[]), 0);
        assert_eq!(
            resolved_text_width(&[None, Some(ipv4)]),
            format!(" ({ipv4})").len()
        );
        assert_eq!(
            resolved_text_width(&[Some(ipv4), Some(ipv6)]),
            format!(" ({ipv6})").len()
        );
    }
}
