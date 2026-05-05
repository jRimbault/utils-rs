//! Pure formatting utilities — functional core, no side effects.
//!
//! All functions here are deterministic and produce only their return value;
//! they never read from the filesystem, write to stderr, or mutate global state.

use ratatui::style::Color;
use std::time::Duration;

/// A percentage value in [0.0, 100.0], used for CPU and memory metrics.
///
/// Centralises the traffic-light colour mapping and ratio conversion that
/// would otherwise be scattered across callers.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Percent(f64);

impl Percent {
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    /// Map this percentage to a traffic-light color for TUI gauges/bars.
    ///
    /// Green < 30 %, Yellow < 70 %, Red otherwise.
    pub fn color(self) -> Color {
        match self.0 {
            p if p < 30.0 => Color::Green,
            p if p < 70.0 => Color::Yellow,
            _ => Color::Red,
        }
    }

    /// Return the raw `f64` value.
    pub fn value(self) -> f64 {
        self.0
    }

    /// Return the value as a ratio in [0.0, 1.0] for use with `bar`.
    pub fn ratio(self) -> f64 {
        self.0 / 100.0
    }
}

/// Render a horizontal bar using braille dot characters.
///
/// Each braille cell represents two vertical positions. The bar fills from
/// the left using the bottom dot row (⣀) and leaves empty cells as the
/// braille blank (⠀), producing a thin baseline-style bar.
///
/// The ratio is taken from `pct.ratio()` and clamped to [0.0, 1.0].
pub fn bar(pct: Percent, width: usize) -> String {
    let ratio = pct.ratio().clamp(0.0, 1.0);
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

/// Build a single-row braille time-series string for `width` terminal columns.
///
/// Each column encodes two consecutive samples (left/right braille dot columns)
/// mapped to 0-4 filled dot rows from the bottom.  The most-recent sample sits
/// at the rightmost column; the trace grows right-to-left as history fills in.
/// A baseline row of bottom dots is always visible so the graph area reads
/// clearly even when all values are zero.
///
/// Takes an iterator of `Percent` values so this function has no dependency
/// on any specific history container type.
pub fn braille_graph(samples: impl IntoIterator<Item = Percent>, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    // Scale by 10 to avoid floating-point comparisons in integer max/ratio math.
    let values: Vec<u64> = samples
        .into_iter()
        .map(|p| (p.value() * 10.0) as u64)
        .collect();
    let max = values.iter().copied().max().unwrap_or(0).max(1);
    let n = 2 * width;
    let take = n.min(values.len());
    let mut slots = vec![0u64; n];
    for (i, &v) in values.iter().rev().take(take).enumerate() {
        slots[n - 1 - i] = v;
    }
    (0..width)
        .map(|col| {
            let lv = slots[2 * col];
            let rv = slots[2 * col + 1];
            // Always show at least 1 dot height for the baseline.
            let lh = ((lv * 4) / max).clamp(1, 4) as usize;
            let rh = ((rv * 4) / max).clamp(1, 4) as usize;
            braille_cell_heights(lh, rh)
        })
        .collect()
}

/// Build a multi-row braille filled-area graph as a `Vec` of row strings.
///
/// Index 0 is the topmost terminal row, index `rows-1` the bottom.  Each
/// terminal row contributes 4 braille dot levels, so `rows` rows yield
/// `rows × 4` distinct fill heights.  Values are scaled to the history maximum
/// and filled from the bottom upward, identical to btop's graph style.
/// The bottom row always shows at least 1 dot height as a visible baseline.
///
/// Takes an iterator of `Percent` values so this function has no dependency
/// on any specific history container type.
pub fn braille_graph_multi(
    samples: impl IntoIterator<Item = Percent>,
    width: usize,
    rows: usize,
) -> Vec<String> {
    if width == 0 || rows == 0 {
        return vec![String::new(); rows];
    }
    let values: Vec<u64> = samples
        .into_iter()
        .map(|p| (p.value() * 10.0) as u64)
        .collect();
    let max = values.iter().copied().max().unwrap_or(0).max(1);
    let n = 2 * width;
    let take = n.min(values.len());
    let mut slots = vec![0u64; n];
    for (i, &v) in values.iter().rev().take(take).enumerate() {
        slots[n - 1 - i] = v;
    }

    let total_levels = (rows as u64) * 4;
    let bottom_row = rows - 1;

    (0..rows)
        .map(|row| {
            // row 0 = topmost terminal row = highest fill levels
            let level_base = (rows - 1 - row) as u64 * 4;
            (0..width)
                .map(|col| {
                    let lv = (slots[2 * col] * total_levels) / max;
                    let rv = (slots[2 * col + 1] * total_levels) / max;
                    let mut lh = lv.saturating_sub(level_base).min(4) as usize;
                    let mut rh = rv.saturating_sub(level_base).min(4) as usize;
                    // Always show a baseline dot in the bottom row.
                    if row == bottom_row {
                        lh = lh.max(1);
                        rh = rh.max(1);
                    }
                    braille_cell_heights(lh, rh)
                })
                .collect()
        })
        .collect()
}

/// Encode one braille cell from per-column fill heights in 0-4.
///
/// Unicode 8-dot braille fill-from-bottom bitmasks (U+2800 base):
/// ```text
/// Row 4 (bottom): left = bit6 (0x40), right = bit7 (0x80)
/// Row 3:          left = bit2 (0x04), right = bit5 (0x20)
/// Row 2:          left = bit1 (0x02), right = bit4 (0x10)
/// Row 1 (top):    left = bit0 (0x01), right = bit3 (0x08)
/// ```
pub fn braille_cell_heights(lh: usize, rh: usize) -> char {
    const LEFT: [u8; 5] = [0x00, 0x40, 0x44, 0x46, 0x47];
    const RIGHT: [u8; 5] = [0x00, 0x80, 0xA0, 0xB0, 0xB8];
    char::from_u32(0x2800 | u32::from(LEFT[lh] | RIGHT[rh])).unwrap_or(' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_empty() {
        assert_eq!(bar(Percent::new(0.0), 8), "⠀⠀⠀⠀⠀⠀⠀⠀");
    }

    #[test]
    fn bar_full() {
        assert_eq!(bar(Percent::new(100.0), 8), "⣤⣤⣤⣤⣤⣤⣤⣤");
    }

    #[test]
    fn bar_clamped_above() {
        assert_eq!(bar(Percent::new(200.0), 4), "⣤⣤⣤⣤");
    }

    #[test]
    fn bar_clamped_below() {
        assert_eq!(bar(Percent::new(-100.0), 4), "⠀⠀⠀⠀");
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
        assert_eq!(Percent::new(15.0).color(), Color::Green);
    }

    #[test]
    fn intensity_yellow() {
        assert_eq!(Percent::new(50.0).color(), Color::Yellow);
    }

    #[test]
    fn intensity_red() {
        assert_eq!(Percent::new(90.0).color(), Color::Red);
    }

    // --- braille_graph tests ---

    #[test]
    fn braille_graph_empty_iterator_produces_width_cells() {
        // An empty sample set should produce exactly `width` braille characters
        // (all baseline cells — lh=1, rh=1 for each column).
        let result = braille_graph(std::iter::empty(), 4);
        assert_eq!(result.chars().count(), 4);
    }

    #[test]
    fn braille_graph_zero_width_returns_empty() {
        let result = braille_graph([Percent::new(50.0)], 0);
        assert_eq!(result, "");
    }

    // --- braille_graph_multi tests ---

    #[test]
    fn braille_graph_multi_row_count_matches() {
        // Output must have exactly `rows` strings regardless of sample count.
        let samples: Vec<Percent> = (0..=10).map(|i| Percent::new(i as f64 * 10.0)).collect();
        let result = braille_graph_multi(samples, 6, 4);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn braille_graph_multi_each_row_has_width_chars() {
        let samples: Vec<Percent> = (0..=10).map(|i| Percent::new(i as f64 * 10.0)).collect();
        let result = braille_graph_multi(samples, 5, 3);
        for row in &result {
            assert_eq!(
                row.chars().count(),
                5,
                "each row must have exactly `width` braille chars"
            );
        }
    }

    #[test]
    fn braille_graph_multi_zero_rows_returns_empty_vec() {
        let result = braille_graph_multi([Percent::new(50.0)], 4, 0);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn braille_graph_multi_monotone_has_nonempty_bottom_row() {
        // With monotonically increasing values, the bottom row must not be all
        // blank — the baseline guarantee ensures at least 1 dot per cell.
        let samples: Vec<Percent> = (1..=20).map(|i| Percent::new(i as f64 * 5.0)).collect();
        let result = braille_graph_multi(samples, 4, 2);
        let bottom = result.last().expect("must have a bottom row");
        // The braille blank is U+2800; any non-blank cell suffices.
        let blank = '\u{2800}';
        assert!(
            bottom.chars().any(|c| c != blank),
            "bottom row should have at least one non-blank cell"
        );
    }
}
