//! Pure rendering helpers for the printer.
//!
//! This module is the functional core of the printer: every function here is
//! side-effect free and depends only on its inputs. It knows nothing about the
//! event loop or the mutable `PrinterState`; it only knows how to turn domain
//! values into styled strings and how to push those strings onto existing
//! progress bars.

use std::net::IpAddr;

use crate::{spinner_style::SpinnerStyle, types};

/// Build a spinner style from the given template, sharing frames and tick
/// interval with the globally-selected `SpinnerStyle`.
pub fn make_style(template: &str, spinner_style: SpinnerStyle) -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::default_spinner()
        .tick_strings(spinner_style.frames())
        .template(template)
        .expect("valid template")
}

/// Decide what address to display next to a host.
///
/// When the user typed an IP literal that resolved to itself, showing the
/// resolved address would be redundant, so we return `None`.
pub fn resolved_addr_for_display(host: &types::Hostname, addr: IpAddr) -> Option<IpAddr> {
    match host.as_str().parse::<IpAddr>() {
        Ok(literal_addr) if literal_addr == addr => None,
        _ => Some(addr),
    }
}

/// Re-render every bar's prefix.
///
/// Used when the resolved-address column grows and every existing prefix needs
/// to be re-aligned to the new width.
pub fn refresh_prefixes(
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

/// Build the prefix shown on the left of a spinner line.
pub fn render_prefix(
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

/// Build the prefix used in the "FAILED" line printed above the spinners.
///
/// The host name is rendered bold so the failure line stands out in the
/// scrollback history.
pub fn render_failure_prefix(
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

/// Left-pad a hostname to `host_width` columns.
pub fn render_host_text(host: &types::Hostname, host_width: usize) -> String {
    format!("{:<host_width$}", host.as_str())
}

/// Width of the "resolved address" column: the widest ` (addr)` seen so far.
pub fn resolved_text_width(resolved_addrs: &[Option<IpAddr>]) -> usize {
    resolved_addrs
        .iter()
        .flatten()
        .map(|addr| format!(" ({addr})").len())
        .max()
        .unwrap_or(0)
}

/// Render the "resolved address" column as ` (addr)` padded to
/// `resolved_width`, or `None` when the column is not displayed at all.
pub fn render_resolved_text(
    resolved_width: usize,
    resolved_addr: Option<IpAddr>,
) -> Option<String> {
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
