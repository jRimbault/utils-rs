//! Human-readable duration display.
//!
//! `HumanDuration` wraps a `std::time::Duration` and implements `Display`
//! with a compact, two-component format:
//!
//! | Range          | Format      | Examples              |
//! |----------------|-------------|-----------------------|
//! | < 60 s         | `{s}s`      | `0s`, `45s`, `59s`    |
//! | 60 s – 59 m    | `{m}m [{s}s]` | `1m`, `2m`, `1m 5s` |
//! | ≥ 60 m         | `{h}h [{m}m]` | `1h`, `1h 30m`      |
//!
//! Seconds are suppressed in the minute tier when they are zero; minutes are
//! suppressed in the hour tier when they are zero.

use std::{fmt, time::Duration};

/// A `Duration` with a compact human-readable `Display` implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HumanDuration(pub Duration);

impl fmt::Display for HumanDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();
        if secs < 60 {
            return write!(f, "{secs}s");
        }
        let (total_minutes, seconds) = (secs / 60, secs % 60);
        if total_minutes < 60 {
            return if seconds == 0 {
                write!(f, "{total_minutes}m")
            } else {
                write!(f, "{total_minutes}m {seconds}s")
            };
        }
        let (hours, minutes) = (total_minutes / 60, total_minutes % 60);
        if minutes == 0 {
            write!(f, "{hours}h")
        } else {
            write!(f, "{hours}h {minutes}m")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(secs: u64) -> String {
        HumanDuration(Duration::from_secs(secs)).to_string()
    }

    #[test]
    fn sub_minute_shows_seconds() {
        assert_eq!(h(0), "0s");
        assert_eq!(h(1), "1s");
        assert_eq!(h(45), "45s");
        assert_eq!(h(59), "59s");
    }

    #[test]
    fn minute_tier_whole_and_partial() {
        assert_eq!(h(60), "1m");
        assert_eq!(h(120), "2m");
        assert_eq!(h(65), "1m 5s");
        assert_eq!(h(3599), "59m 59s");
    }

    #[test]
    fn hour_tier_whole_and_partial() {
        assert_eq!(h(3600), "1h");
        assert_eq!(h(7200), "2h");
        assert_eq!(h(3660), "1h 1m");
        assert_eq!(h(5400), "1h 30m");
        assert_eq!(h(3661), "1h 1m"); // seconds are dropped in hour tier
    }

    #[test]
    fn sub_minute_millis_truncate_to_whole_seconds() {
        assert_eq!(
            HumanDuration(Duration::from_millis(59_900)).to_string(),
            "59s"
        );
    }
}
