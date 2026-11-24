//! The delegate usage log (`delegate-usage.jsonl`), per-task totals, cost, and the daily budgets.
//!
//! Records are appended one per line, so a crash never rewrites earlier records.
//! A token count that a provider did not report is kept as unknown and is never
//! summed as zero. A cost comes only from the local price in config, and a call
//! without a price or without token counts has no cost; it is counted separately.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use super::providers::DEFAULT_MAX_OUTPUT_TOKENS;
use super::{DelegateError, Request};
use crate::config::delegate::{DelegateConfig, DelegateProviderConfig};

/// Name of the usage log under the analysis directory.
pub const USAGE_FILE: &str = "delegate-usage.jsonl";

/// Tokens per price unit. Prices are micro-USD per million tokens.
const TOKENS_PER_PRICE_UNIT: i64 = 1_000_000;

/// One answered call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// RFC 3339, UTC.
    pub at: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub chain: String,
    /// Task folder name for a `tasks run` call; empty for a direct `plaesy delegate` call.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task: String,
    pub billable: bool,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    /// Cost in micro-USD. `None` when the provider has no local price or a token count is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_micro_usd: Option<i64>,
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
    /// Sum of the recorded costs, in micro-USD.
    pub cost_micro_usd: i64,
    #[serde(rename = "calls_without_cost")]
    pub uncosted_calls: u64,
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
        match record.cost_micro_usd {
            Some(cost) => self.cost_micro_usd = self.cost_micro_usd.saturating_add(cost),
            None => self.uncosted_calls += 1,
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
/// line cannot hide the rest of the history. Lines written before `task` and
/// `cost_micro_usd` existed still read, with those fields empty.
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

/// Totals per task. A direct `plaesy delegate` call is grouped under `""`.
#[must_use]
pub fn summarize_by_task(records: &[Record]) -> BTreeMap<String, Totals> {
    let mut by_task: BTreeMap<String, Totals> = BTreeMap::new();
    for record in records {
        by_task.entry(record.task.clone()).or_default().add(record);
    }
    by_task
}

/// Estimated (input, output) tokens for a request: about four characters per prompt token,
/// and the output allowance. It is an estimate, and the budget check is only as good as it is.
fn estimate_split(req: &Request) -> (i64, i64) {
    let output = if req.max_output_tokens == 0 {
        DEFAULT_MAX_OUTPUT_TOKENS
    } else {
        req.max_output_tokens
    };
    ((req.prompt.len() / 4) as i64, i64::from(output))
}

/// Rough pre-call estimate of input plus output tokens. See [`estimate_split`].
#[must_use]
pub fn estimate_tokens(req: &Request) -> i64 {
    let (input, output) = estimate_split(req);
    input.saturating_add(output)
}

/// Cost in micro-USD of `input` and `output` tokens at the local prices. Rounds down.
#[must_use]
pub fn cost_for(input: i64, output: i64, entry: &DelegateProviderConfig) -> i64 {
    input
        .saturating_mul(entry.input_micro_usd_per_mtok)
        .saturating_add(output.saturating_mul(entry.output_micro_usd_per_mtok))
        / TOKENS_PER_PRICE_UNIT
}

/// Records of `provider` on the UTC date of `now`.
fn records_today<'a>(records: &'a [Record], provider: &str, now: DateTime<Utc>) -> Vec<&'a Record> {
    let day = now.format("%Y-%m-%d").to_string();
    records
        .iter()
        .filter(|r| r.provider == provider)
        .filter(|r| {
            DateTime::parse_from_rfc3339(&r.at)
                .is_ok_and(|at| at.with_timezone(&Utc).format("%Y-%m-%d").to_string() == day)
        })
        .collect()
}

/// Tokens recorded for `provider` on the UTC date of `now`. Records without counts add nothing.
#[must_use]
pub fn tokens_on(records: &[Record], provider: &str, now: DateTime<Utc>) -> i64 {
    records_today(records, provider, now)
        .into_iter()
        .filter_map(|r| Some(r.input_tokens?.saturating_add(r.output_tokens?)))
        .fold(0, i64::saturating_add)
}

/// Cost in micro-USD recorded for `provider` on the UTC date of `now`. Uncosted records add nothing.
#[must_use]
pub fn cost_on(records: &[Record], provider: &str, now: DateTime<Utc>) -> i64 {
    records_today(records, provider, now)
        .into_iter()
        .filter_map(|r| r.cost_micro_usd)
        .fold(0, i64::saturating_add)
}

/// Refuses a billable call that would exceed the provider's daily token or cost budget.
/// A budget of 0 is not limited. A cost budget needs a local price, otherwise nothing
/// could ever be counted against it. The refusal is terminal, so a chain stops instead
/// of spending elsewhere. A billable call still needs `--confirm`.
///
/// # Errors
///
/// A terminal [`DelegateError`] when a budget would be exceeded, or a cost budget has no price.
pub fn check_budget(
    entry: &DelegateProviderConfig,
    req: &Request,
    records: &[Record],
    now: DateTime<Utc>,
) -> Result<(), DelegateError> {
    if !entry.billable {
        return Ok(());
    }
    let (input, output) = estimate_split(req);
    if entry.daily_token_budget > 0 {
        let used = tokens_on(records, &entry.name, now);
        let need = input.saturating_add(output);
        if used.saturating_add(need) > entry.daily_token_budget {
            return Err(DelegateError::terminal(format!(
                "provider {:?}: daily token budget of {} would be exceeded (used {used} today, this call needs about {need}); no request was sent",
                entry.name, entry.daily_token_budget
            )));
        }
    }
    if entry.daily_cost_budget_micro_usd > 0 {
        if !entry.is_priced() {
            return Err(DelegateError::terminal(format!(
                "provider {:?}: a daily cost budget is set but no token price is configured; no request was sent",
                entry.name
            )));
        }
        let used = cost_on(records, &entry.name, now);
        let need = cost_for(input, output, entry);
        if used.saturating_add(need) > entry.daily_cost_budget_micro_usd {
            return Err(DelegateError::terminal(format!(
                "provider {:?}: daily cost budget of {} micro-USD would be exceeded (spent {used} micro-USD today, this call needs about {need}); no request was sent",
                entry.name, entry.daily_cost_budget_micro_usd
            )));
        }
    }
    Ok(())
}

/// The configured entry for `provider`. An unknown name gets an unpriced, non-billable placeholder.
#[must_use]
pub fn entry_named(cfg: &DelegateConfig, provider: &str) -> DelegateProviderConfig {
    cfg.providers
        .iter()
        .find(|p| p.name == provider)
        .cloned()
        .unwrap_or_else(|| DelegateProviderConfig {
            name: provider.to_owned(),
            ..Default::default()
        })
}

/// Builds a record for an answered call, timestamped now. The cost comes from `entry`'s
/// local price, and is `None` when the entry is unpriced or a token count is unknown.
#[must_use]
pub fn record_for(
    entry: &DelegateProviderConfig,
    chain: &str,
    task: &str,
    input: Option<u64>,
    output: Option<u64>,
    wall_ms: u64,
) -> Record {
    let input_tokens = input.and_then(|v| i64::try_from(v).ok());
    let output_tokens = output.and_then(|v| i64::try_from(v).ok());
    let cost_micro_usd = match (input_tokens, output_tokens) {
        (Some(i), Some(o)) if entry.is_priced() => Some(cost_for(i, o, entry)),
        _ => None,
    };
    Record {
        at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        provider: entry.name.clone(),
        chain: chain.to_owned(),
        task: task.to_owned(),
        billable: entry.billable,
        input_tokens,
        output_tokens,
        cost_micro_usd,
        wall_ms: i64::try_from(wall_ms).unwrap_or(i64::MAX),
    }
}

#[cfg(test)]
#[path = "usage_test.rs"]
mod usage_test;
