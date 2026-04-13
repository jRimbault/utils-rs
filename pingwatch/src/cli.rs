use crate::types::Hostname;
use std::time::Duration;

/// Ping one or more hosts simultaneously, showing live status in a TUI.
#[derive(clap::Parser)]
#[command(version)]
pub struct Args {
    /// Hosts to ping (1-10 hostnames or IP addresses)
    #[arg(required = true, num_args = 1..=10)]
    pub hosts: Vec<Hostname>,
    /// Interval between pings in milliseconds
    #[arg(short, long, default_value = "1000", value_parser = parse_millis)]
    pub interval: Duration,
    /// Per-ping timeout in milliseconds
    #[arg(short, long, default_value = "2000", value_parser = parse_millis)]
    pub timeout: Duration,
}

fn parse_millis(s: &str) -> Result<Duration, String> {
    let ms: u64 = s
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    if ms == 0 {
        return Err("value must be at least 1ms".to_string());
    }
    Ok(Duration::from_millis(ms))
}
