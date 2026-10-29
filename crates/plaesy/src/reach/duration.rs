//! Go-compatible duration text: `time.ParseDuration` and `time.Duration.String`.
//!
//! Config files written by the Go tool hold durations in these forms, such as `24h`, `500ms`,
//! or `1h30m`. Negative durations are not representable and parse to `None`.

use std::time::Duration;

const UNITS: &[(&str, u128)] = &[
    ("ns", 1),
    ("us", 1_000),
    ("µs", 1_000),
    ("μs", 1_000),
    ("ms", 1_000_000),
    ("s", 1_000_000_000),
    ("m", 60 * 1_000_000_000),
    ("h", 3600 * 1_000_000_000),
];

/// Parses a Go duration such as `300ms`, `2h45m`, or `1.5s`.
#[must_use]
pub fn parse(text: &str) -> Option<Duration> {
    let text = text.trim();
    if text == "0" {
        return Some(Duration::ZERO);
    }
    let rest = text.strip_prefix('+').unwrap_or(text);
    if rest.is_empty() || rest.starts_with('-') {
        return None;
    }
    let mut total: u128 = 0;
    let mut remaining = rest;
    while !remaining.is_empty() {
        let num_end = remaining
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(remaining.len());
        if num_end == 0 {
            return None;
        }
        let value: f64 = remaining[..num_end].parse().ok()?;
        remaining = &remaining[num_end..];
        let unit_end = remaining
            .find(|c: char| c.is_ascii_digit() || c == '.')
            .unwrap_or(remaining.len());
        let unit_text = &remaining[..unit_end];
        let unit_ns = UNITS.iter().find(|(name, _)| *name == unit_text)?.1;
        remaining = &remaining[unit_end..];
        total = total.checked_add((value * unit_ns as f64).round() as u128)?;
    }
    let nanos = u64::try_from(total).ok()?;
    Some(Duration::from_nanos(nanos))
}

/// Formats a duration the way Go's `Duration.String` does: `0s`, `500ms`, `5m0s`, `24h0m0s`.
#[must_use]
pub fn format(d: Duration) -> String {
    let nanos = d.as_nanos();
    if nanos == 0 {
        return "0s".to_owned();
    }
    if nanos < 1_000 {
        return format!("{nanos}ns");
    }
    if nanos < 1_000_000 {
        return format!("{}µs", fraction(nanos, 1_000));
    }
    if nanos < 1_000_000_000 {
        return format!("{}ms", fraction(nanos, 1_000_000));
    }
    let total_secs = nanos / 1_000_000_000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = fraction(
        (total_secs % 60) * 1_000_000_000 + nanos % 1_000_000_000,
        1_000_000_000,
    );
    if hours > 0 {
        format!("{hours}h{minutes}m{secs}s")
    } else if minutes > 0 {
        format!("{minutes}m{secs}s")
    } else {
        format!("{secs}s")
    }
}

/// `value / unit` with trailing zeros trimmed from the fraction.
fn fraction(value: u128, unit: u128) -> String {
    let whole = value / unit;
    let rem = value % unit;
    if rem == 0 {
        return whole.to_string();
    }
    let width = unit.ilog10() as usize;
    let digits = format!("{rem:0width$}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_forms_the_config_uses() {
        assert_eq!(parse("24h"), Some(Duration::from_secs(86_400)));
        assert_eq!(parse("500ms"), Some(Duration::from_millis(500)));
        assert_eq!(parse("1h30m"), Some(Duration::from_secs(5_400)));
        assert_eq!(parse("1.5s"), Some(Duration::from_millis(1_500)));
        assert_eq!(parse("-1s"), None);
        assert_eq!(parse("soon"), None);
    }

    #[test]
    fn formats_like_go() {
        assert_eq!(format(Duration::ZERO), "0s");
        assert_eq!(format(Duration::from_millis(500)), "500ms");
        assert_eq!(format(Duration::from_secs(300)), "5m0s");
        assert_eq!(format(Duration::from_secs(86_400)), "24h0m0s");
        assert_eq!(format(Duration::from_millis(1_500)), "1.5s");
    }
}
