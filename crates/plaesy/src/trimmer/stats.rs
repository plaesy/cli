//! Token estimates and the cumulative savings log (`token-stats.json`).

use std::fs;
use std::io;
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

/// Rough chars-per-token estimate, rounded up. No tokenizer dependency.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Percentage saved from `before` to `after`, rounded to one decimal place.
#[must_use]
pub fn pct_saved(before: usize, after: usize) -> f64 {
    if before == 0 {
        return 0.0;
    }
    let pct = (1.0 - after as f64 / before as f64) * 100.0;
    round_to(pct, 1)
}

fn round_to(value: f64, decimals: u32) -> f64 {
    let mult = 10f64.powi(decimals as i32);
    let sign = if value < 0.0 { -1.0 } else { 1.0 };
    ((value * mult + sign * 0.5).trunc()) / mult
}

/// One record in `token-stats.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatRecord {
    pub timestamp: String,
    pub layer: String,
    pub path: String,
    pub before: usize,
    pub after: usize,
    pub saved_pct: f64,
}

/// Appends one record to the stats file, creating it when needed. A corrupt file starts fresh.
///
/// # Errors
///
/// I/O failures writing the file.
pub fn add_record(
    stats_path: &Path,
    layer: &str,
    target: &str,
    before: usize,
    after: usize,
) -> io::Result<()> {
    if let Some(parent) = stats_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut records: Vec<StatRecord> = fs::read_to_string(stats_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    records.push(StatRecord {
        timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        layer: layer.to_owned(),
        path: target.to_owned(),
        before,
        after,
        saved_pct: pct_saved(before, after),
    });
    let mut text = serde_json::to_string_pretty(&records).map_err(io::Error::other)?;
    text.push('\n');
    fs::write(stats_path, text)
}

/// Cumulative report by layer and in total.
///
/// # Errors
///
/// Never for a missing or empty log, which yields the "no stats yet" message.
pub fn report(stats_path: &Path) -> String {
    const EMPTY: &str = "No token stats yet. Run 'compress' or 'run' first.\n";
    let Some(records) = fs::read_to_string(stats_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<StatRecord>>(&text).ok())
        .filter(|r| !r.is_empty())
    else {
        return EMPTY.to_owned();
    };

    let mut by_layer: std::collections::BTreeMap<String, (usize, usize, usize)> =
        Default::default();
    let (mut grand_before, mut grand_after) = (0, 0);
    for r in &records {
        let entry = by_layer.entry(r.layer.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += r.before;
        entry.2 += r.after;
        grand_before += r.before;
        grand_after += r.after;
    }

    let mut out = String::from("plaesy-trim report\n====================\n");
    for (layer, (runs, before, after)) in &by_layer {
        out.push_str(&format!(
            "{layer}: {runs} runs, {before} -> {after} tokens ({:.1}% avg saved)\n",
            pct_saved(*before, *after)
        ));
    }
    out.push_str("--------------------\n");
    out.push_str(&format!(
        "total: {grand_before} -> {grand_after} tokens ({:.1}% saved)\n",
        pct_saved(grand_before, grand_after)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_round_up_and_savings_round_to_one_decimal() {
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(pct_saved(3, 2), 33.3);
        assert_eq!(pct_saved(0, 0), 0.0);
    }

    #[test]
    fn report_says_when_there_is_nothing_yet() {
        let dir = tempfile::tempdir().unwrap();
        assert!(report(&dir.path().join("none.json")).starts_with("No token stats yet"));
    }

    #[test]
    fn records_accumulate_and_report_totals() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token-stats.json");
        add_record(&path, "layer1", "git status", 100, 50).unwrap();
        add_record(&path, "layer1", "git status", 100, 50).unwrap();
        assert!(report(&path).contains("total: 200 -> 100 tokens (50.0% saved)"));
    }
}
