//! Pings a host in a loop with in-place terminal output.
//!
//! Success messages overwrite the current terminal line via CR + erase-to-EOL.
//! Failure messages are printed on a new persistent line, so they accumulate in
//! the scroll buffer and are never overwritten.
//! Every message is prefixed with the local datetime.

use anyhow::Context;
use chrono::Local;
use clap::Parser;
use std::{
    io::{self, Write},
    net::IpAddr,
    time::Duration,
};
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

const GREEN_OK: &str = "\x1b[32m✓\x1b[0m";
const RED_FAIL: &str = "\x1b[31m✗\x1b[0m";
// CR + erase to end of line
const OVERWRITE: &str = "\r\x1b[K";

/// Ping a host in a loop, updating status in place for successes.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Host to ping (hostname or IP address)
    host: String,
    /// Interval between pings in milliseconds
    #[arg(short, long, default_value = "1000")]
    interval: u64,
    /// Per-ping timeout in milliseconds
    #[arg(short, long, default_value = "1000")]
    timeout: u64,
}

/// Whether the cursor is sitting at the end of an in-place line (no newline yet)
/// or at the start of a fresh line.
#[derive(Clone, Copy, PartialEq)]
enum LineState {
    Fresh,
    InPlace,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let addr = resolve_host(&args.host)
        .await
        .with_context(|| format!("resolving '{}'", args.host))?;

    // Pick the ICMP version to match the resolved address family.
    let icmp_kind = match addr {
        IpAddr::V4(_) => ICMP::V4,
        IpAddr::V6(_) => ICMP::V6,
    };
    let client = Client::new(&Config::builder().kind(icmp_kind).build())
        .context("creating ping client — may need CAP_NET_RAW or root")?;

    // Use the process ID as the ICMP identifier to distinguish from other instances.
    let id = PingIdentifier(std::process::id() as u16);
    let mut pinger = client.pinger(addr, id).await;
    pinger.timeout(Duration::from_millis(args.timeout));

    let mut stdout = io::stdout();
    let mut line_state = LineState::Fresh;
    let mut seq: u16 = 0;

    loop {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

        match pinger.ping(PingSequence(seq), &[0u8; 8]).await {
            Ok((_, rtt)) => {
                let ms = rtt.as_secs_f64() * 1000.0;
                print!("{OVERWRITE}{timestamp} {GREEN_OK} OK  rtt={ms:.1}ms");
                stdout.flush()?;
                line_state = LineState::InPlace;
            }
            Err(e) => {
                if line_state == LineState::InPlace {
                    // Terminate the in-place line so the error starts on a fresh line.
                    println!();
                }
                println!("{timestamp} {RED_FAIL} FAILED: {e}");
                line_state = LineState::Fresh;
            }
        }

        seq = seq.wrapping_add(1);
        tokio::time::sleep(Duration::from_millis(args.interval)).await;
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
