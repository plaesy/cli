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
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        parse_date("2026-10-19").expect("date")
    }

    fn md(v: Value) -> HashMap<String, Value> {
        serde_json::from_value(v).expect("map")
    }

    #[test]
    fn parse_date_rejects_trailing_garbage_after_calendar_date() {
        assert!(parse_date("2026-11-12").is_some());
        assert!(parse_date("2026-11-12garbage").is_none());
        assert!(parse_date("2026-11-12T00:00:00Zjunk").is_none());
    }

    #[test]
    fn tier_aliases_and_path_inference() {
        assert_eq!(normalize_tier(" Knowledge "), Some("semantic"));
        assert_eq!(normalize_tier("daily"), Some("episodic"));
        assert_eq!(normalize_tier("nonsense"), None);
        assert_eq!(infer_tier("10-episodic/2026/10/a.md"), Some("episodic"));
        assert_eq!(infer_tier("20_semantic/people/x.md"), Some("semantic"));
        assert_eq!(infer_tier("00-core/persona.md"), Some("core"));
        // The file name itself never decides the tier.
        assert_eq!(infer_tier("notes/archive.md"), None);
    }

    #[test]
    fn frontmatter_beats_path_and_numbers_map_to_trust() {
        let m = build_meta(
            "10-episodic/a.md",
            &md(json!({"tier": "procedural", "confidence": 0.9})),
            now(),
            180,
        );
        assert_eq!(m.tier.as_deref(), Some("procedural"));
        assert_eq!(m.trust.as_deref(), Some("high"));
        let low = build_meta("a.md", &md(json!({"confidence": "0.2"})), now(), 180);
        assert_eq!(low.trust.as_deref(), Some("low"));
        for invalid in [json!(-0.1), json!(1.1), json!("-0.1"), json!("1.1")] {
            assert_eq!(Trust::parse(&invalid), None, "invalid trust: {invalid}");
        }
        assert_eq!(Trust::parse(&json!(0.0)), Some(Trust::Low));
        assert_eq!(Trust::parse(&json!(1.0)), Some(Trust::High));
        assert_eq!(Trust::parse(&json!("0.5")), Some(Trust::Medium));
    }

    #[test]
    fn staleness_rules() {
        let old = json!({"tier": "semantic", "last_verified": "2026-01-01"});
        assert!(build_meta("a.md", &md(old.clone()), now(), 180).stale);
        assert!(!build_meta("a.md", &md(old), now(), 0).stale, "0 disables");
        let fresh = json!({"tier": "semantic", "last_verified": "2026-10-01"});
        assert!(!build_meta("a.md", &md(fresh), now(), 180).stale);
        let log = json!({"tier": "episodic", "last_verified": "2020-01-01"});
        assert!(
            !build_meta("a.md", &md(log), now(), 180).stale,
            "episodic never stale"
        );
        let unverified = json!({"tier": "semantic"});
        assert!(!build_meta("a.md", &md(unverified), now(), 180).stale);
        assert_eq!(age_days("2026-10-09T10:00:00Z", now()), Some(9));
        assert_eq!(age_days("not a date", now()), None);
    }

    fn res(path: &str, meta: ResultMeta) -> SearchResult {
        SearchResult {
            chunk_id: 1,
            text: "t".into(),
            source_path: path.into(),
            start_line: 1,
            end_line: 1,
            revision: None,
            indexed_at: "2026-10-19T00:00:00Z".into(),
            score: 0.5,
            confidence: crate::memory::storage::ConfidenceLevel::Fact,
            meta,
            explain: None,
        }
    }

    #[test]
    fn project_filter_matches_owner_and_shared_scope_case_insensitively() {
        let md = md(json!({"project": "Studio-A", "scope": ["Studio-B", "Studio-C"]}));
        let metadata = build_meta("a.md", &md, now(), 180);
        assert_eq!(metadata.project.as_deref(), Some("Studio-A"));
        assert_eq!(metadata.scope, vec!["Studio-B", "Studio-C"]);

        for project in ["studio-a", "STUDIO-B"] {
            let filter = SearchFilter {
                project: Some(project.into()),
                ..SearchFilter::default()
            };
            assert!(filter.is_restrictive());
            assert!(filter.accepts(&res("a.md", metadata.clone())));
        }
        let other = SearchFilter {
            project: Some("studio-d".into()),
            ..SearchFilter::default()
        };
        assert!(!other.accepts(&res("a.md", metadata)));
        assert!(SearchFilter::default().accepts(&res("a.md", ResultMeta::default())));
    }

    #[test]
    fn non_string_validity_metadata_is_rejected_by_as_of_filter() {
        let filter = SearchFilter {
            as_of: parse_date("2026-06-15"),
            ..SearchFilter::default()
        };
        for raw in [json!(123), Value::Null, json!(true)] {
            let metadata = md(json!({"valid_from": raw}));
            let result = res("a.md", build_meta("a.md", &metadata, now(), 180));
            assert!(
                !filter.accepts(&result),
                "malformed validity metadata must not be treated as absent: {metadata:?}"
            );
        }
    }

    #[test]
    fn event_date_bounds_are_explicit_and_fail_closed() {
        let in_range = res(
            "a.md",
            build_meta("a.md", &md(json!({"event_date": "2026-06-15"})), now(), 180),
        );
        let filter = SearchFilter {
            event_from: parse_date("2026-06-15"),
            event_until: parse_date("2026-06-16"),
            ..SearchFilter::default()
        };
        assert!(filter.is_restrictive());
        assert!(filter.accepts(&in_range));
        assert!(!SearchFilter {
            event_from: parse_date("2026-06-16"),
            ..SearchFilter::default()
        }
        .accepts(&in_range));
        assert!(!SearchFilter {
            event_until: parse_date("2026-06-15"),
            ..SearchFilter::default()
        }
        .accepts(&in_range));
        assert!(!filter.accepts(&res("undated.md", ResultMeta::default())));
        let malformed = res(
            "bad.md",
            build_meta("bad.md", &md(json!({"event_date": true})), now(), 180),
        );
        assert!(!filter.accepts(&malformed));
    }

    #[test]
    fn index_time_bounds_filter_inclusively_and_fail_closed() {
        let result = res("a.md", ResultMeta::default());
        let day = |value: &str| parse_date(value).expect("valid date");
        let same_day = SearchFilter {
            indexed_since: Some(day("2026-10-19")),
            indexed_until: Some(day("2026-10-20")),
            ..SearchFilter::default()
        };
        assert!(same_day.is_restrictive());
        assert!(same_day.accepts(&result));

        let before = SearchFilter {
            indexed_since: Some(day("2026-10-20")),
            ..SearchFilter::default()
        };
        assert!(!before.accepts(&result));
        let after = SearchFilter {
            indexed_until: Some(day("2026-10-19")),
            ..SearchFilter::default()
        };
        assert!(!after.accepts(&result));

        let mut malformed = result;
        malformed.indexed_at = "not-a-time".into();
        assert!(!same_day.accepts(&malformed));
    }

    #[test]
    fn as_of_keeps_sources_valid_on_that_day() {
        let dated = |from: Option<&str>, to: Option<&str>| ResultMeta {
            valid_from: from.map(Into::into),
            valid_to: to.map(Into::into),
            ..ResultMeta::default()
        };
        let f = SearchFilter {
            as_of: parse_date("2026-06-15"),
            ..SearchFilter::default()
        };
        assert!(f.is_restrictive());
        assert!(f.accepts(&res("a.md", dated(Some("2026-01-01"), Some("2026-12-31")))));
        assert!(
            !f.accepts(&res("a.md", dated(Some("2026-07-01"), None))),
            "not yet valid"
        );
        assert!(
            !f.accepts(&res("a.md", dated(None, Some("2026-06-01")))),
            "already expired"
        );
        assert!(f.accepts(&res("a.md", dated(None, None))), "undated passes");
        assert!(
            !f.accepts(&res("a.md", dated(Some("not-a-date"), None))),
            "invalid validity start must not pass an as-of filter"
        );
        assert!(
            !f.accepts(&res("a.md", dated(None, Some("not-a-date")))),
            "invalid validity end must not pass an as-of filter"
        );
        // Date-only bounds include the whole calendar day, even at its final instant.
        let end_of_day = SearchFilter {
            as_of: parse_date("2026-06-15T23:59:59Z"),
            ..SearchFilter::default()
        };
        assert!(end_of_day.accepts(&res("a.md", dated(None, Some("2026-06-15")))));
        assert!(end_of_day.accepts(&res("a.md", dated(None, Some("2026-06-15T23:59:59Z")))));
        let after_timestamp_end = SearchFilter {
            as_of: parse_date("2026-06-15T23:59:59.000001Z"),
            ..SearchFilter::default()
        };
        assert!(
            !after_timestamp_end.accepts(&res("a.md", dated(None, Some("2026-06-15T23:59:59Z"))))
        );

        let m = build_meta(
            "a.md",
            &md(json!({"valid_from": "2026-01-01", "valid_to": "2026-06-30"})),
            now(),
            180,
        );
        assert_eq!(m.valid_from.as_deref(), Some("2026-01-01"));
        assert_eq!(m.valid_to.as_deref(), Some("2026-06-30"));
    }

    #[test]
    fn boolean_tag_expressions_parse_and_match_exact_declared_tags() {
        let tags = vec!["Rust".to_string(), "memory".to_string()];
        for (expression, expected) in [
            ("rust AND memory", true),
            ("rust AND NOT archived", true),
            ("(rust OR cli) AND memory", true),
            ("Rust OR cli AND absent", true),
            ("rust and missing", false),
            ("NOT archived", true),
            ("RUST", true),
            ("rusty", false),
        ] {
            let parsed = TagExpr::parse(expression).unwrap_or_else(|e| panic!("{expression}: {e}"));
            assert_eq!(parsed.matches(&tags), expected, "{expression}");
        }
        let filter = SearchFilter {
            tag_filter: Some(TagExpr::parse("rust AND memory").expect("valid expression")),
            ..SearchFilter::default()
        };
        assert!(filter.is_restrictive());
        assert!(filter.accepts(&res(
            "a.md",
            ResultMeta {
                tags,
                ..ResultMeta::default()
            }
        )));
    }

    #[test]
    fn boolean_tag_expressions_reject_invalid_or_excessive_input() {
        for invalid in ["", "rust AND", "(rust OR cli", "rust xor cli", "rust/tag"] {
            assert!(TagExpr::parse(invalid).is_err(), "accepted {invalid:?}");
        }
        assert!(TagExpr::parse(&"a ".repeat(65)).is_err());
        assert!(TagExpr::parse(&format!("{}a{}", "(".repeat(33), ")".repeat(33))).is_err());
        assert!(TagExpr::parse(&"a ".repeat(600)).is_err());
    }

    #[test]
    fn tag_filter_matching_is_case_insensitive() {
        let meta = ResultMeta {
            tags: vec!["Rust".into(), "Memory".into()],
            ..ResultMeta::default()
        };
        let any = SearchFilter {
            tags: vec!["rust".into()],
            ..SearchFilter::default()
        };
        let all = SearchFilter {
            tags: vec!["RUST".into(), "memory".into()],
            tags_all: true,
            ..SearchFilter::default()
        };
        assert!(any.accepts(&res("a.md", meta.clone())));
        assert!(all.accepts(&res("a.md", meta)));
    }

    #[test]
    fn tag_filter_matches_any_by_default_and_all_when_requested() {
        let tagged = ResultMeta {
            tags: vec!["rust".into(), "memory".into()],
            ..ResultMeta::default()
        };
        let any = SearchFilter {
            tags: vec!["rust".into(), "cli".into()],
            ..SearchFilter::default()
        };
        assert!(any.accepts(&res("a.md", tagged.clone())));
        assert!(!any.accepts(&res("a.md", ResultMeta::default())));
        let all = SearchFilter {
            tags: vec!["rust".into(), "cli".into()],
            tags_all: true,
            ..SearchFilter::default()
        };
        assert!(!all.accepts(&res("a.md", tagged)));
    }

    #[test]
    fn filter_semantics() {
        let sem = ResultMeta {
            tier: Some("semantic".into()),
            trust: Some("medium".into()),
            ..ResultMeta::default()
        };
        let old = ResultMeta {
            status: Some("superseded".into()),
            ..sem.clone()
        };
        let all = SearchFilter::default();
        assert!(all.accepts(&res("a.md", sem.clone())));
        assert!(
            !all.accepts(&res("a.md", old.clone())),
            "superseded hidden by default"
        );
        let uppercase_inactive = ResultMeta {
            status: Some("ARCHIVED".into()),
            ..sem.clone()
        };
        assert!(
            !all.accepts(&res("a.md", uppercase_inactive)),
            "inactive status matching is case-insensitive"
        );
        let inc = SearchFilter {
            include_inactive: true,
            ..SearchFilter::default()
        };
        assert!(inc.accepts(&res("a.md", old)));
        for status in ["deprecated", "archived", "obsolete"] {
            let inactive = ResultMeta {
                status: Some(status.into()),
                ..sem.clone()
            };
            assert!(
                !all.accepts(&res("a.md", inactive.clone())),
                "{status} should be hidden by default"
            );
            assert!(
                inc.accepts(&res("a.md", inactive)),
                "{status} should appear when include_inactive is true"
            );
        }

        let by_tier = SearchFilter {
            tiers: vec!["knowledge".into()], // alias of semantic
            ..SearchFilter::default()
        };
        assert!(by_tier.accepts(&res("a.md", sem.clone())));
        assert!(
            !by_tier.accepts(&res("a.md", ResultMeta::default())),
            "no tier never matches"
        );

        let trusted = SearchFilter {
            min_trust: Some(Trust::High),
            ..SearchFilter::default()
        };
        assert!(!trusted.accepts(&res("a.md", sem.clone())));
        assert!(
            !trusted.accepts(&res("a.md", ResultMeta::default())),
            "unknown trust excluded"
        );
        let ok = ResultMeta {
            trust: Some("high".into()),
            ..sem
        };
        assert!(trusted.accepts(&res("a.md", ok)));

        let scoped = SearchFilter {
            path_prefix: Some("20-semantic/".into()),
            ..SearchFilter::default()
        };
        assert!(scoped.accepts(&res("20-semantic\\x.md", ResultMeta::default())));
        assert!(scoped.accepts(&res("20-semantic/x.md", ResultMeta::default())));
        assert!(scoped.accepts(&res("20-semantic", ResultMeta::default())));
        assert!(!scoped.accepts(&res("20-semantics/x.md", ResultMeta::default())));
        assert!(!scoped.accepts(&res("20-semantic-old/x.md", ResultMeta::default())));
        assert!(!scoped.accepts(&res("10-episodic/x.md", ResultMeta::default())));
        let empty_scope = SearchFilter {
            path_prefix: Some(String::new()),
            ..SearchFilter::default()
        };
        assert!(empty_scope.accepts(&res("anywhere/note.md", ResultMeta::default())));
    }
}
