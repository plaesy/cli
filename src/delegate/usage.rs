//! The delegate usage log (`delegate-usage.jsonl`) and the daily token budget.
//!
//! Records are appended one per line, so a crash never rewrites earlier records.
//! A token count that a provider did not report is kept as unknown and is never
//! summed as zero.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use super::providers::DEFAULT_MAX_OUTPUT_TOKENS;
use super::{DelegateError, Request};
use crate::config::delegate::DelegateProviderConfig;

/// Name of the usage log under the analysis directory.
pub const USAGE_FILE: &str = "delegate-usage.jsonl";

/// One answered call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// RFC 3339, UTC.
    pub at: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub chain: String,
    pub billable: bool,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub wall_ms: i64,
}

/// Totals over a set of records.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Totals {
    pub calls: u64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    #[serde(rename = "calls_without_token_counts")]
    pub unknown_tokens: u64,
    pub wall_ms: i64,
}

impl Totals {
    fn add(&mut self, record: &Record) {
        self.calls += 1;
        self.wall_ms += record.wall_ms;
        match (record.input_tokens, record.output_tokens) {
            (Some(input), Some(output)) => {
                self.input_tokens += input;
                self.output_tokens += output;
            }
            _ => self.unknown_tokens += 1,
        }
    }
}

/// Path of the usage log under an analysis directory.
#[must_use]
pub fn usage_path(analysis_dir: &Path) -> PathBuf {
    analysis_dir.join(USAGE_FILE)
}

/// Appends one record, creating the file and its directory when needed.
///
/// # Errors
///
/// I/O failures opening or writing the log.
pub fn append(path: &Path, record: &Record) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(record).map_err(io::Error::other)?;
    writeln!(file, "{line}")
}

/// Reads the log. A missing file is an empty log. A damaged line is skipped, so one bad
/// line cannot hide the rest of the history.
///
/// # Errors
///
/// I/O failures other than a missing file.
pub fn read(path: &Path) -> io::Result<Vec<Record>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut records = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if let Ok(record) = serde_json::from_str::<Record>(&line) {
            if !record.provider.is_empty() {
                records.push(record);
            }
        }
    }
    Ok(records)
}

/// Totals per provider and per chain. A call made without a chain is grouped under `""`.
#[must_use]
pub fn summarize(records: &[Record]) -> (BTreeMap<String, Totals>, BTreeMap<String, Totals>) {
    let mut by_provider: BTreeMap<String, Totals> = BTreeMap::new();
    let mut by_chain: BTreeMap<String, Totals> = BTreeMap::new();
    for record in records {
        by_provider
            .entry(record.provider.clone())
            .or_default()
            .add(record);
        by_chain
            .entry(record.chain.clone())
            .or_default()
            .add(record);
    }
    (by_provider, by_chain)
}

/// Rough pre-call estimate: about four characters per token for the prompt, plus the
/// output allowance. It is an estimate, and the budget check is only as good as it is.
#[must_use]
pub fn estimate_tokens(req: &Request) -> i64 {
    let output = if req.max_output_tokens == 0 {
        DEFAULT_MAX_OUTPUT_TOKENS
    } else {
        req.max_output_tokens
    };
    (req.prompt.len() / 4) as i64 + i64::from(output)
}

/// Tokens recorded for `provider` on the UTC date of `now`. Records without counts add nothing.
#[must_use]
pub fn tokens_on(records: &[Record], provider: &str, now: DateTime<Utc>) -> i64 {
    let day = now.format("%Y-%m-%d").to_string();
    records
        .iter()
        .filter(|r| r.provider == provider)
        .filter_map(|r| {
            let at = DateTime::parse_from_rfc3339(&r.at).ok()?;
            (at.with_timezone(&Utc).format("%Y-%m-%d").to_string() == day)
                .then_some(r.input_tokens?.saturating_add(r.output_tokens?))
        })
        .sum()
}

/// Refuses a billable call that would exceed the provider's daily budget. A provider
/// with no budget (0) is not limited here, and a billable call still needs `--confirm`.
/// The refusal is terminal, so a chain stops instead of spending elsewhere.
///
/// # Errors
///
/// A terminal [`DelegateError`] when the budget would be exceeded.
pub fn check_budget(
    entry: &DelegateProviderConfig,
    req: &Request,
    records: &[Record],
    now: DateTime<Utc>,
) -> Result<(), DelegateError> {
    if !entry.billable || entry.daily_token_budget <= 0 {
        return Ok(());
    }
    let used = tokens_on(records, &entry.name, now);
    let need = estimate_tokens(req);
    if used.saturating_add(need) > entry.daily_token_budget {
        return Err(DelegateError::terminal(format!(
            "provider {:?}: daily token budget of {} would be exceeded (used {used} today, this call needs about {need}); no request was sent",
            entry.name, entry.daily_token_budget
        )));
    }
    Ok(())
}

/// Builds a record for an answered call, timestamped now.
#[must_use]
pub fn record_for(
    provider: &str,
    chain: &str,
    billable: bool,
    input: Option<u64>,
    output: Option<u64>,
    wall_ms: u64,
) -> Record {
    Record {
        at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        provider: provider.to_owned(),
        chain: chain.to_owned(),
        billable,
        input_tokens: input.and_then(|v| i64::try_from(v).ok()),
        output_tokens: output.and_then(|v| i64::try_from(v).ok()),
        wall_ms: i64::try_from(wall_ms).unwrap_or(i64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn record(provider: &str, input: Option<i64>, output: Option<i64>, at: &str) -> Record {
        Record {
            at: at.into(),
            provider: provider.into(),
            chain: String::new(),
            billable: true,
            input_tokens: input,
            output_tokens: output,
            wall_ms: 10,
        }
    }

    #[test]
    fn unknown_counts_are_not_summed_as_zero() {
        let recs = [
            record("p", Some(100), Some(20), "2026-10-29T01:00:00Z"),
            record("p", None, None, "2026-10-29T02:00:00Z"),
        ];
        let (by_provider, _) = summarize(&recs);
        let totals = &by_provider["p"];
        assert_eq!(totals.calls, 2);
        assert_eq!(totals.input_tokens, 100);
        assert_eq!(totals.unknown_tokens, 1);
    }

    #[test]
    fn budget_counts_only_todays_tokens_for_that_provider() {
        let now = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        let recs = [
            record("p", Some(400), Some(400), "2026-10-29T03:00:00Z"),
            record("p", Some(9000), Some(9000), "2026-10-28T03:00:00Z"),
            record("other", Some(9000), Some(9000), "2026-10-29T03:00:00Z"),
        ];
        assert_eq!(tokens_on(&recs, "p", now), 800);
    }

    #[test]
    fn a_billable_call_over_budget_is_refused_before_sending() {
        let now = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        let entry = DelegateProviderConfig {
            name: "p".into(),
            billable: true,
            daily_token_budget: 100,
            ..Default::default()
        };
        let req = Request {
            prompt: "x".repeat(40),
            max_output_tokens: 200,
            ..Request::default()
        };
        let err = check_budget(&entry, &req, &[], now).unwrap_err();
        assert!(err.message.contains("would be exceeded"));
    }

    #[test]
    fn append_then_read_round_trips_and_skips_damaged_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join(USAGE_FILE);
        append(
            &path,
            &record("p", Some(1), Some(2), "2026-10-29T00:00:00Z"),
        )
        .unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"not json\n")
            .unwrap();
        append(&path, &record("q", None, None, "2026-10-29T00:00:01Z")).unwrap();
        let back = read(&path).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].provider, "q");
    }
}
