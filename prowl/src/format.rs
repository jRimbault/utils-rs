//! Pure formatting utilities — functional core, no side effects.
//!
//! All functions here are deterministic and produce only their return value;
//! they never read from the filesystem, write to stderr, or mutate global state.

use ratatui::style::Color;
use std::time::Duration;

/// Render a horizontal bar using braille dot characters.
///
/// Each braille cell represents two vertical positions. The bar fills from
/// the left using the bottom dot row (⣀) and leaves empty cells as the
/// braille blank (⠀), producing a thin baseline-style bar.
///
/// `ratio` is clamped to [0.0, 1.0] before use.
pub fn bar(ratio: f64, width: usize) -> String {
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let empty = width - filled;
    // ⣤ = bottom two dots filled (U+28E4), ⠀ = braille blank (U+2800)
    format!("{}{}", "⣤".repeat(filled), "⠀".repeat(empty))
}

/// Map a single-character process state to its full descriptive word.
pub fn state_word(state: char) -> &'static str {
    match state {
        'R' => "Running",
        'S' => "Sleeping",
        'D' => "Disk Sleep",
        'Z' => "Zombie",
        'T' => "Stopped",
        't' => "Tracing",
        'X' | 'x' => "Dead",
        'K' => "Wakekill",
        'W' => "Waking",
        'P' => "Parked",
        'I' => "Idle",
        _ => "Unknown",
    }
}

/// Human-readable byte count with IEC prefixes (KiB, MiB, GiB).
pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    match bytes {
        b if b < KIB => format!("{b} B"),
        b if b < MIB => format!("{:.1} KiB", b as f64 / KIB as f64),
        b if b < GIB => format!("{:.1} MiB", b as f64 / MIB as f64),
        b => format!("{:.1} GiB", b as f64 / GIB as f64),
    }
}

/// Format a duration as `Dd HH:MM` (when ≥1 day) or `HH:MM:SS`.
pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if days > 0 {
        format!("{days}d {:02}:{:02}", hours, mins)
    } else {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    }
}

/// Map a percentage value to a traffic-light color for TUI gauges/bars.
///
/// Green < 30 %, Yellow < 70 %, Red otherwise.
pub fn intensity_color(pct: f64) -> Color {
    match pct {
        p if p < 30.0 => Color::Green,
        p if p < 70.0 => Color::Yellow,
        _ => Color::Red,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_empty() {
        assert_eq!(bar(0.0, 8), "⠀⠀⠀⠀⠀⠀⠀⠀");
    }

    #[test]
    fn bar_full() {
        assert_eq!(bar(1.0, 8), "⣤⣤⣤⣤⣤⣤⣤⣤");
    }

    #[test]
    fn bar_clamped_above() {
        assert_eq!(bar(2.0, 4), "⣤⣤⣤⣤");
    }

    #[test]
    fn bar_clamped_below() {
        assert_eq!(bar(-1.0, 4), "⠀⠀⠀⠀");
    }

    #[test]
    fn state_word_sleeping() {
        assert_eq!(state_word('S'), "Sleeping");
    }

    #[test]
    fn state_word_running() {
        assert_eq!(state_word('R'), "Running");
    }

    #[test]
    fn format_bytes_small() {
        assert_eq!(format_bytes(512), "512 B");
    }

    #[test]
    fn format_bytes_kib() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
    }

    #[test]
    fn format_bytes_mib() {
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(3661)), "01:01:01");
    }

    #[test]
    fn format_duration_days() {
        assert_eq!(
            format_duration(Duration::from_secs(86400 + 3600)),
            "1d 01:00"
        );
    }

    #[test]
    fn intensity_green() {
        assert_eq!(intensity_color(15.0), Color::Green);
    }

    #[test]
    fn intensity_yellow() {
        assert_eq!(intensity_color(50.0), Color::Yellow);
    }

    #[test]
    fn intensity_red() {
        assert_eq!(intensity_color(90.0), Color::Red);
    }
}
