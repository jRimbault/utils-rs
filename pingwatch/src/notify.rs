//! Desktop notifications via `notify-send`.
//!
//! Each public function is `async` and awaits the `notify-send` subprocess to
//! completion. The *decision* of when to alert lives in the printer state; this
//! module only performs the side effect. The caller is responsible for
//! dispatching each call in the background (e.g. with `tokio::spawn`). Spawn
//! failures (e.g. `notify-send` not installed) are intentionally ignored so a
//! missing notification daemon never disrupts the live ping display.

use std::{net::IpAddr, time::Duration};

use crate::duration::HumanDuration;

/// Alert that a host has stopped responding for at least `downtime`.
///
/// `downtime` is the elapsed time since the first failure of the current
/// outage, so the message reflects how long the host has actually been silent.
pub async fn host_down(host: String, addr: Option<IpAddr>, downtime: Duration) {
    spawn_notify(
        "critical",
        &format!("{} is unreachable", target(&host, addr)),
        &format!("No ping response for {}.", HumanDuration(downtime)),
    )
    .await;
}

/// Alert that a previously-down host is answering pings again.
pub async fn host_recovered(host: String, addr: Option<IpAddr>) {
    spawn_notify(
        "normal",
        &format!("{} is reachable again", target(&host, addr)),
        "Responding to pings.",
    )
    .await;
}

/// Combine a hostname with its resolved address for display, omitting the
/// address when there is nothing extra to show (IP literals resolve to self).
fn target(host: &str, addr: Option<IpAddr>) -> String {
    match addr {
        Some(addr) => format!("{host} ({addr})"),
        None => host.to_string(),
    }
}

/// Spawn `notify-send` and await its exit so finished notification processes
/// don't linger as zombies.
async fn spawn_notify(urgency: &str, summary: &str, body: &str) {
    let spawn = tokio::process::Command::new("notify-send")
        .arg("--app-name=pingwatch")
        .arg("--urgency")
        .arg(urgency)
        .arg(summary)
        .arg(body)
        .spawn();
    if let Ok(mut child) = spawn {
        let _ = child.wait().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
