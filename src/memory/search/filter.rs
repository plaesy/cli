//! Metadata-aware retrieval: tiers, declared source trust, lifecycle status and staleness.
//!
//! Sources declare these in YAML frontmatter:
//!
//! ```yaml
//! ---
//! tier: semantic            # core | episodic | semantic | procedural | archive
//! confidence: high          # high | medium | low (or a number 0-1)
//! last_verified: 2026-10-01 # when the claims were last checked against reality
//! status: active            # active | superseded | deprecated | archived
//! ---
//! ```
//!
//! `tier` falls back to the path (`10-episodic/…`, `knowledge/…`). Filters are applied to
//! the candidate lists *before* fusion and truncation, so a restrictive filter does not
//! starve the result list. Stale sources (see `SearchConfig::stale_after_days`) are not
//! hidden: they are flagged and down-weighted, so the caller can still see them but a
//! stale-but-confident note no longer outranks a fresh one by default.

use super::SearchEngine;
use crate::memory::error::Result;
use crate::memory::storage::{ResultMeta, SearchResult};
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use std::collections::HashMap;

/// The memory tiers a document can belong to.
pub const TIERS: [&str; 5] = ["core", "episodic", "semantic", "procedural", "archive"];

/// Statuses that mark a source as no longer current. Hidden unless `include_inactive`.
const INACTIVE_STATUSES: [&str; 4] = ["superseded", "deprecated", "archived", "obsolete"];

/// Normalise a tier name (accepts common aliases). Returns `None` for unknown names.
pub fn normalize_tier(name: &str) -> Option<&'static str> {
    match name.trim().to_lowercase().as_str() {
        "core" | "working" | "pinned" => Some("core"),
        "episodic" | "episode" | "daily" | "journal" | "log" => Some("episodic"),
        "semantic" | "knowledge" | "facts" | "fact" => Some("semantic"),
        "procedural" | "procedure" | "skills" | "skill" | "playbook" => Some("procedural"),
        "archive" | "archived" => Some("archive"),
        _ => None,
    }
}

/// Infer a tier from a document path by looking for tier-like path segments.
pub fn infer_tier(path: &str) -> Option<&'static str> {
    path.replace('\\', "/")
        .to_lowercase()
        .split('/')
        .rev()
        .skip(1) // directories only, nearest first
        .find_map(|seg| {
            let seg = seg.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_');
            normalize_tier(seg)
        })
}

/// Declared trust of a source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Trust {
    /// Partially drifted or unverified claims.
    Low,
    /// Not verifiable cheaply.
    Medium,
    /// Verified recently.
    High,
}

impl Trust {
    /// Parse `high|medium|low` (case-insensitive) or a number in 0-1.
    pub fn parse(v: &Value) -> Option<Trust> {
        match v {
            Value::String(s) => match s.trim().to_lowercase().as_str() {
                "high" => Some(Trust::High),
                "medium" | "med" => Some(Trust::Medium),
                "low" => Some(Trust::Low),
                other => other.parse::<f64>().ok().and_then(Self::from_score),
            },
            Value::Number(n) => n.as_f64().and_then(Self::from_score),
            _ => None,
        }
    }

    fn from_score(x: f64) -> Option<Trust> {
        if !(0.0..=1.0).contains(&x) {
            return None;
        }
        Some(if x >= 0.8 {
            Trust::High
        } else if x >= 0.5 {
            Trust::Medium
        } else {
            Trust::Low
        })
    }

    /// Lower-case label.
    pub fn as_str(self) -> &'static str {
        match self {
            Trust::High => "high",
            Trust::Medium => "medium",
            Trust::Low => "low",
        }
    }
}

/// Boolean expression over declared tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagExpr {
    /// A literal tag, matched case-insensitively and exactly.
    Tag(String),
    /// Negation.
    Not(Box<TagExpr>),
    /// Conjunction.
    And(Box<TagExpr>, Box<TagExpr>),
    /// Disjunction.
    Or(Box<TagExpr>, Box<TagExpr>),
}

impl TagExpr {
    /// Parse a tag expression. `NOT` binds tighter than `AND`, which binds tighter than `OR`.
    pub fn parse(input: &str) -> std::result::Result<Self, String> {
        if input.len() > 1024 {
            return Err("tag expression exceeds the 1024-byte limit".into());
        }
        let tokens = lex_tag_expression(input)?;
        if tokens.is_empty() {
            return Err("tag expression is empty".into());
        }
        let mut parser = TagParser {
            tokens,
            position: 0,
            depth: 0,
        };
        let expr = parser.parse_or()?;
        if parser.position != parser.tokens.len() {
            return Err(format!(
                "unexpected token '{}' in tag expression",
                parser.tokens[parser.position].display()
            ));
        }
        Ok(expr)
    }

    fn matches(&self, tags: &[String]) -> bool {
        match self {
            Self::Tag(wanted) => tags.iter().any(|tag| tag.eq_ignore_ascii_case(wanted)),
            Self::Not(inner) => !inner.matches(tags),
            Self::And(left, right) => left.matches(tags) && right.matches(tags),
            Self::Or(left, right) => left.matches(tags) || right.matches(tags),
        }
    }
}

#[derive(Clone, Debug)]
enum TagToken {
    Ident(String),
    And,
    Or,
    Not,
    Left,
    Right,
}
impl TagToken {
    fn display(&self) -> &str {
        match self {
            Self::Ident(s) => s,
            Self::And => "AND",
            Self::Or => "OR",
            Self::Not => "NOT",
            Self::Left => "(",
            Self::Right => ")",
        }
    }
}

fn lex_tag_expression(input: &str) -> std::result::Result<Vec<TagToken>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        let token = match ch {
            '(' => TagToken::Left,
            ')' => TagToken::Right,
            c if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':' => {
                let mut ident = String::from(c);
                while let Some((_, next)) = chars.peek().copied() {
                    if next.is_ascii_alphanumeric() || matches!(next, '_' | '-' | '.' | ':') {
                        ident.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match ident.to_ascii_uppercase().as_str() {
                    "AND" => TagToken::And,
                    "OR" => TagToken::Or,
                    "NOT" => TagToken::Not,
                    _ => TagToken::Ident(ident),
                }
            }
            _ => return Err(format!("invalid character '{ch}' in tag expression")),
        };
        tokens.push(token);
        if tokens.len() > 128 {
            return Err("tag expression exceeds the 128-token limit".into());
        }
    }
    Ok(tokens)
}

struct TagParser {
    tokens: Vec<TagToken>,
    position: usize,
    depth: usize,
}
impl TagParser {
    fn parse_or(&mut self) -> std::result::Result<TagExpr, String> {
        let mut expr = self.parse_and()?;
        while matches!(self.tokens.get(self.position), Some(TagToken::Or)) {
            self.position += 1;
            expr = TagExpr::Or(Box::new(expr), Box::new(self.parse_and()?));
        }
        Ok(expr)
    }
    fn parse_and(&mut self) -> std::result::Result<TagExpr, String> {
        let mut expr = self.parse_unary()?;
        while matches!(self.tokens.get(self.position), Some(TagToken::And)) {
            self.position += 1;
            expr = TagExpr::And(Box::new(expr), Box::new(self.parse_unary()?));
        }
        Ok(expr)
    }
    fn parse_unary(&mut self) -> std::result::Result<TagExpr, String> {
        if matches!(self.tokens.get(self.position), Some(TagToken::Not)) {
            self.position += 1;
            return Ok(TagExpr::Not(Box::new(self.parse_unary()?)));
        }
        match self.tokens.get(self.position).cloned() {
            Some(TagToken::Ident(tag)) => {
                self.position += 1;
                Ok(TagExpr::Tag(tag))
            }
            Some(TagToken::Left) => {
                self.depth += 1;
                if self.depth > 32 {
                    return Err("tag expression exceeds the 32-level nesting limit".into());
                }
                self.position += 1;
                let inner = self.parse_or()?;
                if !matches!(self.tokens.get(self.position), Some(TagToken::Right)) {
                    return Err("missing ')' in tag expression".into());
                }
                self.position += 1;
                self.depth -= 1;
                Ok(inner)
            }
            Some(token) => Err(format!(
                "expected a tag, NOT, or '('; found '{}'",
                token.display()
            )),
            None => Err("expected a tag in tag expression".into()),
        }
    }
}

/// Restricts which sources may appear in results.
#[derive(Clone, Debug, Default)]
pub struct SearchFilter {
    /// Keep only these tiers (empty = all). Documents without a tier never match a tier filter.
    pub tiers: Vec<String>,

    /// Keep only notes owned by or shared with this project ID.
    pub project: Option<String>,

    /// Keep documents matching these tags (empty = all); any tag matches by default.
    pub tags: Vec<String>,

    /// Require every requested tag instead of any requested tag.
    pub tags_all: bool,

    /// Optional boolean expression over exact, case-insensitive declared tags.
    pub tag_filter: Option<TagExpr>,

    /// Keep only documents whose path starts with this prefix.
    pub path_prefix: Option<String>,

    /// Keep only sources declaring at least this trust. Sources declaring none are excluded.
    pub min_trust: Option<Trust>,

    /// Also return superseded / deprecated / archived sources.
    pub include_inactive: bool,

    /// Keep only sources whose declared validity period contains this instant ("as of" a date).
    /// A source with no `valid_from` / `valid_to` is undated and always passes this check.
    pub as_of: Option<DateTime<Utc>>,

    /// Keep only results indexed at or after this instant (date-only values mean start of day).
    pub indexed_since: Option<DateTime<Utc>>,

    /// Keep only results indexed before this instant (date-only values are next-day exclusive).
    pub indexed_until: Option<DateTime<Utc>>,

    /// Keep only sources whose explicit event date is on or after this instant.
    pub event_from: Option<DateTime<Utc>>,

    /// Keep only sources whose explicit event date is before this exclusive bound.
    pub event_until: Option<DateTime<Utc>>,
}

impl SearchFilter {
    /// `true` if the filter can discard candidates beyond the always-on status check.
    pub fn is_restrictive(&self) -> bool {
        !self.tiers.is_empty()
            || self.project.is_some()
            || !self.tags.is_empty()
            || self.tag_filter.is_some()
            || self.path_prefix.is_some()
            || self.min_trust.is_some()
            || self.as_of.is_some()
            || self.indexed_since.is_some()
            || self.indexed_until.is_some()
            || self.event_from.is_some()
            || self.event_until.is_some()
    }

    /// Whether `r` (with `meta` already filled) passes the filter.
    pub fn accepts(&self, r: &SearchResult) -> bool {
        if !self.include_inactive {
            if let Some(status) = &r.meta.status {
                if INACTIVE_STATUSES
                    .iter()
                    .any(|inactive| status.eq_ignore_ascii_case(inactive))
                {
                    return false;
                }
            }
        }
        if let Some(project) = self.project.as_deref() {
            let matches_project = r
                .meta
                .project
                .as_deref()
                .is_some_and(|owner| owner.eq_ignore_ascii_case(project))
                || r.meta
                    .scope
                    .iter()
                    .any(|scope| scope.eq_ignore_ascii_case(project));
            if !matches_project {
                return false;
            }
        }
        if !self.tiers.is_empty() {
            let wanted = |t: &str| {
                self.tiers
                    .iter()
                    .any(|w| normalize_tier(w) == normalize_tier(t) && normalize_tier(t).is_some())
            };
            match &r.meta.tier {
                Some(t) if wanted(t) => {}
                _ => return false,
            }
        }
        if let Some(expression) = &self.tag_filter {
            if !expression.matches(&r.meta.tags) {
                return false;
            }
        }
        if !self.tags.is_empty() {
            let wanted = |tag: &str| {
                self.tags
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(tag))
            };
            let matches = if self.tags_all {
                self.tags.iter().all(|candidate| {
                    r.meta
                        .tags
                        .iter()
                        .any(|tag| candidate.eq_ignore_ascii_case(tag))
                })
            } else {
                r.meta.tags.iter().any(|tag| wanted(tag))
            };
            if !matches {
                return false;
            }
        }
        if let Some(prefix) = &self.path_prefix {
            let path = r.source_path.replace('\\', "/");
            let prefix = prefix.replace('\\', "/");
            let prefix = prefix.trim_end_matches('/');
            if !prefix.is_empty()
                && path != prefix
                && !path
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('/'))
            {
                return false;
            }
        }
        if let Some(min) = self.min_trust {
            let have = r
                .meta
                .trust
                .as_deref()
                .and_then(|t| Trust::parse(&Value::String(t.to_string())));
            if have.is_none_or(|t| t < min) {
                return false;
            }
        }
        if let Some(at) = self.as_of {
            let from = match r.meta.valid_from.as_deref() {
                Some(raw) => match parse_date(raw) {
                    Some(parsed) => Some(parsed),
                    None => return false,
                },
                None => None,
            };
            let to = match r.meta.valid_to.as_deref() {
                Some(raw) => match parse_date(raw) {
                    Some(parsed) => Some((raw, parsed)),
                    None => return false,
                },
                None => None,
            };
            let expired = to.is_some_and(|(raw, to)| {
                if raw.trim().len() == 10 {
                    // Date-only bounds include the entire calendar day.
                    at.date_naive() > to.date_naive()
                } else {
                    at > to
                }
            });
            if from.is_some_and(|f| f > at) || expired {
                return false;
            }
        }
        if self.indexed_since.is_some() || self.indexed_until.is_some() {
            let Some(indexed_at) = parse_date(&r.indexed_at) else {
                return false;
            };
            if self.indexed_since.is_some_and(|since| indexed_at < since)
                || self.indexed_until.is_some_and(|until| indexed_at >= until)
            {
                return false;
            }
        }
        if self.event_from.is_some() || self.event_until.is_some() {
            let Some(event_date) = r.meta.event_date.as_deref().and_then(parse_date) else {
                return false;
            };
            if self.event_from.is_some_and(|from| event_date < from)
                || self.event_until.is_some_and(|until| event_date >= until)
            {
                return false;
            }
        }
        true
    }
}

/// Parse `YYYY-MM-DD` or an RFC 3339 timestamp.
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&Utc));
    }
    if s.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc())
}

/// Whole days between `last_verified` and `now` (None if unparseable).
pub fn age_days(last_verified: &str, now: DateTime<Utc>) -> Option<i64> {
    parse_date(last_verified).map(|d| (now - d).num_days())
}

/// Build result metadata from a document's frontmatter and path.
pub fn build_meta(
    path: &str,
    md: &HashMap<String, Value>,
    now: DateTime<Utc>,
    stale_after_days: u32,
) -> ResultMeta {
    let text = |k: &str| {
        md.get(k)
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
    };
    let tier = text("tier")
        .and_then(|t| normalize_tier(&t))
        .or_else(|| infer_tier(path))
        .map(str::to_string);
    let trust = md
        .get("confidence")
        .or_else(|| md.get("trust"))
        .and_then(Trust::parse)
        .map(|t| t.as_str().to_string());
    let last_verified = text("last_verified");
    let status = text("status").map(|s| s.to_lowercase());
    let project = text("project");
    let scope = md
        .get("scope")
        .map(|value| match value {
            Value::String(value) => vec![value.trim().to_string()],
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .map(|value| value.trim().to_string())
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default();
    let tags = md
        .get("tags")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    // Preserve malformed declared bounds so `--as-of` can fail closed instead of
    // treating non-string YAML values as if the validity bound were absent.
    let validity = |key: &str| {
        md.get(key).map(|value| {
            value
                .as_str()
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        })
    };
    let valid_from = validity("valid_from");
    let valid_to = validity("valid_to");
    let event_date = validity("event_date");
    // Episodic logs and archives record what was true then; they do not go stale.
    let ages = !matches!(tier.as_deref(), Some("episodic") | Some("archive"));
    let stale = ages
        && stale_after_days > 0
        && last_verified
            .as_deref()
            .and_then(|lv| age_days(lv, now))
            .is_some_and(|d| d > i64::from(stale_after_days));
    ResultMeta {
        tier,
        tags,
        trust,
        last_verified,
        status,
        project,
        scope,
        valid_from,
        valid_to,
        event_date,
        stale,
    }
}

impl SearchEngine {
    /// Attach source metadata to `results`, apply the staleness penalty, drop results the
    /// filter rejects and re-sort by score. Called on each candidate list before fusion.
    pub(super) async fn refine(
        &self,
        mut results: Vec<SearchResult>,
        filter: &SearchFilter,
    ) -> Result<Vec<SearchResult>> {
        if results.is_empty() {
            return Ok(results);
        }
        let mut paths: Vec<String> = results.iter().map(|r| r.source_path.clone()).collect();
        paths.sort();
        paths.dedup();
        let docs = self.db.document_metadata(&paths).await?;
        let now = Utc::now();
        let empty = HashMap::new();
        for r in &mut results {
            let md = docs.get(&r.source_path).unwrap_or(&empty);
            r.meta = build_meta(&r.source_path, md, now, self.config.stale_after_days);
            if r.meta.stale {
                r.score *= self.config.stale_penalty;
            }
        }
        results.retain(|r| filter.accepts(r));
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(results)
    }
}

#[cfg(test)]
#[path = "filter_test.rs"]
mod filter_test;
