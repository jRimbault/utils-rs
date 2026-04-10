//! Pings one or more hosts simultaneously, showing live per-host status via
//! an indicatif MultiProgress TUI.
//!
//! Each host occupies one spinner row that is updated in place on success.
//! Failures are printed as persistent lines above the spinners so they
//! accumulate in the scroll buffer and are never overwritten.

use anyhow::Context;
use chrono::Local;
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{net::IpAddr, time::Duration};
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

const GREEN_OK: &str = "\x1b[32mOK\x1b[0m";
const RED_FAIL: &str = "\x1b[31mFAILED\x1b[0m";
const YELLOW_WAIT: &str = "\x1b[33mwaiting\x1b[0m";

/// Ping one or more hosts simultaneously, showing live status in a TUI.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Hosts to ping (1–10 hostnames or IP addresses)
    #[arg(required = true, num_args = 1..=10)]
    hosts: Vec<String>,
    /// Interval between pings in milliseconds
    #[arg(short, long, default_value = "1000")]
    interval: u64,
    /// Per-ping timeout in milliseconds
    #[arg(short, long, default_value = "1000")]
    timeout: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let multi = MultiProgress::new();

    // Column width is the length of the longest hostname so the status
    // column is as tight as possible while still staying aligned.
    let prefix_width = args.hosts.iter().map(|h| h.len()).max().unwrap_or(0);
    let template = format!("{{spinner:.cyan}} {{prefix:<{prefix_width}}} {{msg}}");
    let style = ProgressStyle::default_spinner()
        .tick_chars("✶✸✹✺✹✷")
        .template(&template)
        .expect("valid template");

    // Derive a unique ICMP identifier per host from the process ID so
    // concurrent pingers don't conflict with each other.
    let base_id = std::process::id() as u16;
    let interval = Duration::from_millis(args.interval);
    let timeout = Duration::from_millis(args.timeout);

    let mut tasks = Vec::with_capacity(args.hosts.len());

    for (i, host) in args.hosts.into_iter().enumerate() {
        let pb = multi.add(ProgressBar::new_spinner());
        pb.set_style(style.clone());
        pb.set_prefix(host.clone());
        pb.set_message("resolving…");
        // Animate the spinner independently of the ping interval.
        pb.enable_steady_tick(Duration::from_millis(80));

        let id = PingIdentifier(base_id.wrapping_add(i as u16));

        tasks.push(tokio::spawn(ping_loop(
            host,
            id,
            pb,
            multi.clone(),
            interval,
            timeout,
        )));
    }

    // All tasks loop forever; awaiting them keeps main alive until Ctrl-C.
    for task in tasks {
        let _ = task.await;
    }

    Ok(())
}

/// Resolves the host, then pings it in a loop, updating `pb` in place for
/// successes and printing persistent failure lines via `multi`.
async fn ping_loop(
    host: String,
    id: PingIdentifier,
    pb: ProgressBar,
    multi: MultiProgress,
    interval: Duration,
    timeout: Duration,
) {
    let addr = match resolve_host(&host).await {
        Ok(a) => {
            pb.set_message(format!("resolved → {a}"));
            a
        }
        Err(e) => {
            pb.finish_with_message(format!("resolution failed: {e}"));
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
            pb.finish_with_message(format!("client error: {e}"));
            return;
        }
    };

    let mut pinger = client.pinger(addr, id).await;
    pinger.timeout(timeout);

    let mut seq: u16 = 0;

    loop {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

        match pinger.ping(PingSequence(seq), &[0u8; 8]).await {
            Ok((_, rtt)) => {
                let ms = rtt.as_secs_f64() * 1000.0;
                pb.set_message(format!("{GREEN_OK}  rtt={ms:.1}ms"));
            }
            Err(e) => {
                // Persistent line: accumulates above the spinner rows.
                let _ = multi.println(format!("{timestamp} {host} {RED_FAIL}: {e}"));
                pb.set_message(YELLOW_WAIT.to_string());
            }
        }

        seq = seq.wrapping_add(1);
        tokio::time::sleep(interval).await;
    }
}

/// Resolves a hostname or IP string to an `IpAddr`.
/// IP addresses are parsed directly; hostnames go through async DNS lookup.
async fn resolve_host(host: &str) -> anyhow::Result<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }
    let mut addrs = tokio::net::lookup_host(format!("{host}:0"))
        .await
        .with_context(|| format!("DNS lookup for '{host}'"))?;
    addrs
        .next()
        .map(|sa| sa.ip())
        .ok_or_else(|| anyhow::anyhow!("no addresses found for '{host}'"))
}
