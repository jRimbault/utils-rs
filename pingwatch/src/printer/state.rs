//! Mutable printer state.
//!
//! This module owns the set of progress bars and the bookkeeping that tracks
//! per-host status (resolved address, ok/waiting style, column widths). It
//! applies one `PingEvent` at a time via `handle`, delegating all string
//! construction to `super::render`. It does not know about the tokio event
//! loop or channels.

use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use super::render;
use crate::{event, notify, spinner_style::SpinnerStyle, types};

const WAIT_TEMPLATE: &str = "{spinner:.red} {prefix} {msg}";
const OK_TEMPLATE: &str = "{spinner:.green} {prefix} rtt={msg}";

/// Aggregates the indicatif bars and the derived state needed to keep their
/// prefixes aligned as new addresses are resolved.
pub(super) struct PrinterState {
    multi: indicatif::MultiProgress,
    bars: Vec<indicatif::ProgressBar>,
    hosts: Arc<[types::Hostname]>,
    host_width: usize,
    style_ok: indicatif::ProgressStyle,
    style_wait: indicatif::ProgressStyle,
    bar_is_ok: Vec<bool>,
    resolved_addrs: Vec<Option<IpAddr>>,
    resolved_width: usize,
    /// How long a host must be silent before a desktop notification fires.
    notify_after: Duration,
    /// Start of the current consecutive-failure streak per host, or `None`
    /// while the host is responding. Used to measure outage duration.
    down_since: Vec<Option<Instant>>,
    /// Wall-clock start of the current outage per host, for display purposes.
    down_since_wall: Vec<Option<chrono::DateTime<chrono::Local>>>,
    /// Whether a "host down" notification has already been sent for the
    /// current outage, so we alert once per outage rather than every ping.
    notified_down: Vec<bool>,
}

impl PrinterState {
    /// Build a `PrinterState` with one spinner per host in "resolving..." state.
    pub(super) fn new(
        hosts: Arc<[types::Hostname]>,
        spinner_style: SpinnerStyle,
        notify_after: Duration,
    ) -> Self {
        let multi = indicatif::MultiProgress::new();
        let host_width = hosts.iter().map(|h| h.as_str().len()).max().unwrap_or(0);
        let style_ok = render::make_style(OK_TEMPLATE, spinner_style);
        let style_wait = render::make_style(WAIT_TEMPLATE, spinner_style);

        let bars: Vec<indicatif::ProgressBar> = hosts
            .iter()
            .map(|host| {
                let pb = multi.add(indicatif::ProgressBar::new_spinner());
                pb.set_style(style_wait.clone());
                pb.set_prefix(render::render_prefix(host, host_width, 0, None));
                pb.set_message("resolving...");
                pb
            })
            .collect();

        let n = bars.len();
        Self {
            multi,
            bars,
            hosts,
            host_width,
            style_ok,
            style_wait,
            bar_is_ok: vec![false; n],
            resolved_addrs: vec![None; n],
            resolved_width: 0,
            notify_after,
            down_since: vec![None; n],
            down_since_wall: vec![None; n],
            notified_down: vec![false; n],
        }
    }

    /// Apply a single event to the bar identified by its `idx`. Events whose
    /// index is out of range are silently ignored, preserving robustness when
    /// the upstream channel briefly becomes inconsistent with the host list.
    pub(super) fn handle(&mut self, ev: event::PingEvent) {
        let i = ev.idx().as_usize();
        if self.bars.get(i).is_none() {
            return;
        }
        match ev {
            event::PingEvent::Resolved { addr, .. } => self.on_resolved(i, addr),
            event::PingEvent::ResolutionFailed { error, .. } => self.on_resolution_failed(i, error),
            event::PingEvent::Success { rtt, .. } => self.on_success(i, rtt),
            event::PingEvent::Failure { error, .. } => self.on_failure(i, error),
        }
    }

    /// Advance every spinner by one frame.
    #[cfg(feature = "animated-spinners")]
    pub(super) fn tick(&self) {
        for bar in &self.bars {
            bar.tick();
        }
    }

    fn on_resolved(&mut self, i: usize, addr: IpAddr) {
        let display_addr = render::resolved_addr_for_display(&self.hosts[i], addr);
        self.resolved_addrs[i] = display_addr;
        let next_resolved_width = render::resolved_text_width(&self.resolved_addrs);
        // A growing resolved-address column forces us to re-render every
        // prefix so columns stay aligned; otherwise only this bar changes.
        if next_resolved_width != self.resolved_width {
            self.resolved_width = next_resolved_width;
            render::refresh_prefixes(
                &self.bars,
                &self.hosts,
                self.host_width,
                self.resolved_width,
                &self.resolved_addrs,
            );
        } else {
            self.bars[i].set_prefix(render::render_prefix(
                &self.hosts[i],
                self.host_width,
                self.resolved_width,
                display_addr,
            ));
        }
        self.bars[i].set_message("resolved");
    }

    fn on_resolution_failed(&mut self, i: usize, error: types::ResolveError) {
        self.bars[i].finish_with_message(format!("resolution failed: {error}"));
    }

    fn on_success(&mut self, i: usize, rtt: Duration) {
        // A successful ping ends any outage. If we had alerted that the host
        // was down, tell the user it recovered, then clear the streak so the
        // next outage is measured fresh.
        if self.notified_down[i] {
            tokio::spawn(notify::host_recovered(
                self.hosts[i].as_str().to_string(),
                self.resolved_addrs[i],
            ));
        }
        self.down_since[i] = None;
        self.down_since_wall[i] = None;
        self.notified_down[i] = false;

        let ms = rtt.as_secs_f64() * 1000.0;
        if !self.bar_is_ok[i] {
            self.bars[i].set_style(self.style_ok.clone());
            self.bar_is_ok[i] = true;
        }
        self.bars[i].set_message(format!("{ms:.1}ms"));
    }

    fn on_failure(&mut self, i: usize, error: event::PingFailure) {
        // Track outage duration: the streak starts at the first failure and
        // is re-checked on every subsequent failure so the notification fires
        // once the host has been silent for at least `notify_after`.
        let started = *self.down_since[i].get_or_insert_with(Instant::now);
        let started_wall = *self.down_since_wall[i].get_or_insert_with(chrono::Local::now);
        let elapsed = started.elapsed();
        if !self.notified_down[i] && elapsed >= self.notify_after {
            tokio::spawn(notify::host_down(
                self.hosts[i].as_str().to_string(),
                self.resolved_addrs[i],
                elapsed,
            ));
            self.notified_down[i] = true;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let prefix = render::render_failure_prefix(
            &self.hosts[i],
            self.host_width,
            self.resolved_width,
            self.resolved_addrs[i],
        );
        let _ = self.multi.println(format!(
            "{}  {}  {}  {error}",
            console::style(timestamp).dim(),
            prefix,
            console::style("FAILED").red().bold(),
        ));
        if self.bar_is_ok[i] {
            self.bars[i].set_style(self.style_wait.clone());
            self.bar_is_ok[i] = false;
        }
        self.bars[i].set_message(format!(
            "down since {} ({})",
            format_outage_start(started_wall, chrono::Local::now()),
            crate::duration::HumanDuration(elapsed)
        ));
    }
}

/// Format an outage start time relative to `now`: time only when the outage
/// began today, otherwise prefixed with the date so long outages stay
/// unambiguous.
fn format_outage_start(
    started: chrono::DateTime<chrono::Local>,
    now: chrono::DateTime<chrono::Local>,
) -> String {
    if started.date_naive() == now.date_naive() {
        started.format("%H:%M:%S").to_string()
    } else {
        started.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::format_outage_start;

    #[test]
    fn same_day_shows_time_only() {
        let started = chrono::Local
            .with_ymd_and_hms(2026, 10, 5, 0, 14, 21)
            .unwrap();
        let now = chrono::Local
            .with_ymd_and_hms(2026, 10, 5, 8, 0, 0)
            .unwrap();
        assert_eq!(format_outage_start(started, now), "00:14:21");
    }

    #[test]
    fn earlier_day_shows_date() {
        let started = chrono::Local
            .with_ymd_and_hms(2026, 10, 3, 0, 14, 21)
            .unwrap();
        let now = chrono::Local
            .with_ymd_and_hms(2026, 10, 5, 8, 0, 0)
            .unwrap();
        assert_eq!(format_outage_start(started, now), "2026-10-03 00:14:21");
    }
}
