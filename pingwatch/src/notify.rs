//! Desktop notifications via `notify-send`.
//!
//! This is the imperative shell for alerting: it spawns `notify-send`
//! subprocesses and reaps them. The *decision* of when to alert lives in the
//! printer state; this module only performs the side effect. Spawn failures
//! (e.g. `notify-send` not installed) are intentionally ignored so a missing
//! notification daemon never disrupts the live ping display.

use std::{net::IpAddr, time::Duration};

/// Alert that a host has stopped responding for at least `downtime`.
///
/// `downtime` is the elapsed time since the first failure of the current
/// outage, so the message reflects how long the host has actually been silent.
pub fn host_down(host: &str, addr: Option<IpAddr>, downtime: Duration) {
    spawn_notify(
        "critical",
        &format!("{} is unreachable", target(host, addr)),
        &format!("No ping response for {}.", format_duration(downtime)),
    );
}

/// Alert that a previously-down host is answering pings again.
pub fn host_recovered(host: &str, addr: Option<IpAddr>) {
    spawn_notify(
        "normal",
        &format!("{} is reachable again", target(host, addr)),
        "Responding to pings.",
    );
}

/// Combine a hostname with its resolved address for display, omitting the
/// address when there is nothing extra to show (IP literals resolve to self).
fn target(host: &str, addr: Option<IpAddr>) -> String {
    match addr {
        Some(addr) => format!("{host} ({addr})"),
        None => host.to_string(),
    }
}

/// Spawn `notify-send` and reap it asynchronously so finished notification
/// processes don't linger as zombies for the lifetime of the pinger.
fn spawn_notify(urgency: &str, summary: &str, body: &str) {
    let spawn = tokio::process::Command::new("notify-send")
        .arg("--app-name=pingwatch")
        .arg("--urgency")
        .arg(urgency)
        .arg(summary)
        .arg(body)
        .spawn();
    if let Ok(mut child) = spawn {
        tokio::spawn(async move {
            let _ = child.wait().await;
        });
    }
}

/// Render a downtime duration as a compact human string (`45s`, `2m`, `1m 5s`).
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        return format!("{secs}s");
    }
    let (minutes, seconds) = (secs / 60, secs % 60);
    if seconds == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sub_minute_downtime_in_seconds() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_millis(59_900)), "59s");
    }

    #[test]
    fn formats_whole_and_partial_minutes() {
        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(120)), "2m");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
    }

    #[test]
    fn target_omits_address_when_absent() {
        let addr: IpAddr = "93.184.216.34".parse().unwrap();
        assert_eq!(target("example.com", None), "example.com");
        assert_eq!(
            target("example.com", Some(addr)),
            "example.com (93.184.216.34)"
        );
    }
}
