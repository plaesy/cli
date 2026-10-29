//! Judges a check's output against the value it must reach.
//!
//! An expected value is either a binary condition (`pass`, `zero`, `all`, …)
//! or a comparison against one number (`>= 9`, `<= 2 s`). The same rules apply
//! wherever a checklist runs.
//! Port of `scripts/internal/expected` (Go).

use std::sync::OnceLock;

use regex::Regex;

/// How a judged check ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Met,
    NotMet,
    NotMeasurable,
}

/// Result of [`judge`]: the outcome, the value compared, and a reason when the
/// outcome is `NotMeasurable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judgement {
    pub outcome: Outcome,
    pub actual: String,
    pub reason: String,
}

/// Expected values that [`judge`] accepts. A failing check names them, so a bad
/// expected value is visible in the history.
pub const ACCEPTED_VALUES: &str = "pass, yes, true, present (exit code 0 alone); zero, none; no, false, absent; all, every (pass, or N of N); a comparison such as \">= 9\" or \"<= 2 s\"";

/// Judges a check that exited 0. A `pass` expectation is met by the exit code
/// alone. Anything else is judged by `output`, the last line the command printed.
#[must_use]
pub fn judge(expected: &str, output: &str) -> Judgement {
    if binary_kind(expected) == Some(Binary::Pass) {
        return Judgement {
            outcome: Outcome::Met,
            actual: "pass".into(),
            reason: String::new(),
        };
    }
    if output.is_empty() {
        return Judgement {
            outcome: Outcome::NotMeasurable,
            actual: String::new(),
            reason: "command printed nothing to compare with the expected".into(),
        };
    }
    if let Some(kind) = binary_kind(expected) {
        let outcome = if binary_satisfies(kind, output) {
            Outcome::Met
        } else {
            Outcome::NotMet
        };
        return Judgement {
            outcome,
            actual: output.into(),
            reason: String::new(),
        };
    }
    match numeric_satisfies(expected, output) {
        Numeric::Problem(reason) => Judgement {
            outcome: Outcome::NotMeasurable,
            actual: output.into(),
            reason,
        },
        Numeric::Ok(true) => Judgement {
            outcome: Outcome::Met,
            actual: output.into(),
            reason: String::new(),
        },
        // Not a comparison this grammar knows: the caller reports it as not met.
        Numeric::Ok(false) => Judgement {
            outcome: Outcome::NotMet,
            actual: output.into(),
            reason: String::new(),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Binary {
    Zero,
    Pass,
    Absent,
    All,
}

fn binary_kind(expected: &str) -> Option<Binary> {
    let t = expected.trim().to_lowercase();
    match t.as_str() {
        "zero" | "none" => return Some(Binary::Zero),
        "pass" | "yes" | "true" | "present" => return Some(Binary::Pass),
        "no" | "false" | "absent" => return Some(Binary::Absent),
        _ => {}
    }
    re_all_word().is_match(&t).then_some(Binary::All)
}

fn binary_satisfies(kind: Binary, actual: &str) -> bool {
    let a = actual.trim().to_lowercase();
    match kind {
        Binary::Zero => matches!(a.as_str(), "0" | "none"),
        Binary::Pass => matches!(a.as_str(), "pass" | "yes" | "true" | "present"),
        Binary::Absent => matches!(a.as_str(), "0" | "none" | "no" | "false" | "absent"),
        Binary::All => {
            a == "pass"
                || re_of_count()
                    .captures(&a)
                    .is_some_and(|m| m[1] == m[2] && &m[1] != "0")
        }
    }
}

enum Numeric {
    /// Comparison decided, with the verdict.
    Ok(bool),
    /// Comparison could not be made, with the reason.
    Problem(String),
}

/// Compares a numeric `actual` against a numeric `expected`. Units must match,
/// except that time units convert (ms, s, min, h).
fn numeric_satisfies(expected: &str, actual: &str) -> Numeric {
    let normalized = expected.replace('≥', ">=").replace('≤', "<=");
    let Some(t) = re_num_expected().captures(&normalized) else {
        return Numeric::Ok(false);
    };
    let op = &t[1];
    let Ok(mut want) = t[2].parse::<f64>() else {
        return Numeric::Ok(false);
    };
    let t_unit = t[3].to_owned();

    let Some(a) = re_num_actual().captures(actual) else {
        return Numeric::Problem(
            "actual must be a number with the expected value's unit, for example 1.5 s for an expected value in s".into(),
        );
    };
    let digits = a[1].replace(',', "");
    let fraction = a.get(2).map_or("", |m| m.as_str());
    let Ok(mut got) = format!("{digits}{fraction}").parse::<f64>() else {
        return Numeric::Problem("actual is not a readable number".into());
    };
    let a_unit = a[3].to_owned();

    if t_unit != a_unit {
        match (time_factor(&t_unit), time_factor(&a_unit)) {
            (Some(want_factor), Some(got_factor)) => {
                got *= got_factor;
                want *= want_factor;
            }
            _ => {
                return Numeric::Problem(format!(
                    "units differ: expected {t_unit:?}, actual {a_unit:?}"
                ))
            }
        }
    }
    Numeric::Ok(compare(op, got, want))
}

/// Milliseconds per unit, for the time units the grammar converts between.
fn time_factor(unit: &str) -> Option<f64> {
    match unit.to_lowercase().as_str() {
        "ms" => Some(1.0),
        "s" | "sec" | "secs" => Some(1000.0),
        "min" | "mins" => Some(60_000.0),
        "h" | "hr" | "hrs" => Some(3_600_000.0),
        _ => None,
    }
}

fn compare(op: &str, got: f64, want: f64) -> bool {
    match op {
        ">=" | "≥" => got >= want,
        "<=" | "≤" => got <= want,
        ">" => got > want,
        "<" => got < want,
        _ => got == want,
    }
}

fn re_num_expected() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*(>=|≥|<=|≤|==|=|>|<)\s*([0-9]+(?:\.[0-9]+)?)\s*([A-Za-z%]*)\s*$")
            .expect("static regex")
    })
}

fn re_num_actual() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*((?:[0-9]{1,3}(?:,[0-9]{3})+)|[0-9]+)(\.[0-9]+)?\s*([A-Za-z%]*)\s*$")
            .expect("static regex")
    })
}

fn re_all_word() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(all|every)(\s+\S.*)?$").expect("static regex"))
}

fn re_of_count() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*([0-9]+)\s+of\s+([0-9]+)\s*$").expect("static regex"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(expected: &str, output: &str) -> Outcome {
        judge(expected, output).outcome
    }

    #[test]
    fn binary_expectations() {
        assert_eq!(outcome("pass", ""), Outcome::Met);
        assert_eq!(outcome("zero", "0"), Outcome::Met);
        assert_eq!(outcome("zero", "3"), Outcome::NotMet);
        assert_eq!(outcome("all", "5 of 5"), Outcome::Met);
        assert_eq!(outcome("all", "4 of 5"), Outcome::NotMet);
        assert_eq!(outcome("all pages", "pass"), Outcome::Met);
        assert_eq!(outcome("every route", "3 of 4"), Outcome::NotMet);
    }

    #[test]
    fn numeric_comparisons_and_units() {
        assert_eq!(outcome(">= 9", "9"), Outcome::Met);
        assert_eq!(outcome(">= 9", "8"), Outcome::NotMet);
        assert_eq!(outcome("<= 2 s", "1.5 s"), Outcome::Met);
        assert_eq!(outcome("<= 2 s", "1500 ms"), Outcome::Met);
        assert_eq!(outcome("<= 2 s", "3"), Outcome::NotMeasurable);
        assert_eq!(outcome(">= 9", "1,000"), Outcome::Met);
    }

    #[test]
    fn empty_output_is_not_measurable() {
        assert_eq!(outcome(">= 9", ""), Outcome::NotMeasurable);
    }

    #[test]
    fn sentence_expectation_is_never_met() {
        let j = judge("exit code 2 while running; turn ends when complete", "ok");
        assert_eq!(j.outcome, Outcome::NotMet);
        assert_eq!(j.actual, "ok");
    }
}
