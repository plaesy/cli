//! Machine fixes for the fixable rules. The fixer works on text rather than on
//! violations, so one pass converges. Its line predicates come from the same
//! masks the checks use, so the two cannot disagree about which lines are in scope.

use super::doc::Doc;
use super::text_rules::{is_indented, Md010Options, Md012Options, Md022Options};
use super::RuleSet;

/// Blank-line insertions: a blank goes before each line in `before` and after each in `after`.
#[derive(Default)]
struct Plan {
    before: std::collections::BTreeSet<usize>,
    after: std::collections::BTreeSet<usize>,
}

/// Applies the fixable rules that are enabled in `set`, returning the new source.
#[must_use]
pub fn fix_source(set: &RuleSet, src: &str) -> String {
    let normalized = src.replace("\r\n", "\n");
    let mut lines: Vec<String> = normalized.split('\n').map(str::to_owned).collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    if lines.is_empty() {
        return src.to_owned();
    }

    let enabled = |id: &str| set.rules.iter().any(|r| r.id == id);
    let mut d = Doc::default();
    d.set_lines(lines.clone());

    if enabled("MD010") {
        let o = set.opt::<Md010Options>("MD010");
        let include_code = o.include_code_blocks.unwrap_or(true);
        let spaces = o.spaces_per_tab.unwrap_or(4);
        for (i, line) in lines.iter_mut().enumerate() {
            if d.in_front_matter(i) || (d.in_code(i) && !include_code) {
                continue;
            }
            if line.contains('\t') {
                *line = line.replace('\t', &" ".repeat(spaces));
            }
        }
    }

    if enabled("MD009") {
        let allowed = set
            .opt::<super::text_rules::Md009Options>("MD009")
            .br_spaces
            .unwrap_or(0);
        for (i, line) in lines.iter_mut().enumerate() {
            if d.in_front_matter(i) {
                continue;
            }
            let trimmed = line.trim_end_matches([' ', '\t']).to_owned();
            if line.len() - trimmed.len() > allowed {
                *line = trimmed;
            }
        }
    }

    if enabled("MD012") {
        let max = set.opt::<Md012Options>("MD012").maximum.unwrap_or(1);
        lines = collapse_blank_runs(&lines, &d, max);
    }

    d.set_lines(lines.clone());

    if enabled("MD022") {
        let o = set.opt::<Md022Options>("MD022");
        let (above, below) = (o.lines_above.unwrap_or(1), o.lines_below.unwrap_or(1));
        lines = apply_spacing(lines, &mut d, |doc| plan_md022(doc, above, below));
    }
    if enabled("MD031") {
        lines = apply_spacing(lines, &mut d, plan_md031);
    }
    if enabled("MD058") {
        lines = apply_spacing(lines, &mut d, plan_md058);
    }
    if enabled("MD032") {
        lines = apply_spacing(lines, &mut d, plan_md032);
    }

    let mut out = lines.join("\n");
    if enabled("MD047") {
        out = format!("{}\n", out.trim_end_matches('\n'));
    }
    out
}

fn collapse_blank_runs(lines: &[String], d: &Doc, maximum: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut run = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if !d.in_front_matter(i) && line.trim().is_empty() {
            run += 1;
            if run > maximum {
                continue;
            }
        } else {
            run = 0;
        }
        out.push(line.clone());
    }
    out
}

/// Computes a plan, applies the "before" half, recomputes it, and applies the "after" half.
fn apply_spacing(lines: Vec<String>, d: &mut Doc, plan: impl Fn(&Doc) -> Plan) -> Vec<String> {
    let mut lines = lines;
    let first = plan(d);
    if !first.before.is_empty() {
        lines = insert_blanks_before(&lines, d, &first.before);
        d.set_lines(lines.clone());
    }
    let second = plan(d);
    if !second.after.is_empty() {
        lines = insert_blanks_after(&lines, d, &second.after);
        d.set_lines(lines.clone());
    }
    lines
}

fn insert_blanks_before(
    lines: &[String],
    d: &Doc,
    at: &std::collections::BTreeSet<usize>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + at.len());
    for (i, line) in lines.iter().enumerate() {
        let needs = i > 0
            && at.contains(&i)
            && !d.in_front_matter(i)
            && !d.in_code(i - 1)
            && out.last().is_some_and(|prev| !prev.trim().is_empty());
        if needs {
            out.push(String::new());
        }
        out.push(line.clone());
    }
    out
}

fn insert_blanks_after(
    lines: &[String],
    d: &Doc,
    at: &std::collections::BTreeSet<usize>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + at.len());
    for (i, line) in lines.iter().enumerate() {
        out.push(line.clone());
        let needs = i + 1 < lines.len()
            && at.contains(&i)
            && !d.in_front_matter(i + 1)
            && !d.in_code(i + 1)
            && !lines[i + 1].trim().is_empty();
        if needs {
            out.push(String::new());
        }
    }
    out
}

fn plan_md022(d: &Doc, above: usize, below: usize) -> Plan {
    let mut plan = Plan::default();
    if above > 0 {
        for i in 0..d.n() {
            if d.heading[i] == 0 {
                continue;
            }
            let first_content = i == 0 || (d.fm_end > 0 && i == d.fm_end);
            if !first_content && !d.lines[i - 1].trim().is_empty() {
                plan.before.insert(i);
            }
        }
    }
    if below > 0 {
        for i in 0..d.n() {
            if d.heading[i] == 0 {
                continue;
            }
            let end = i.max(d.heading_end[i]);
            if end + 1 < d.n() && !d.lines[end + 1].trim().is_empty() && d.heading[end + 1] == 0 {
                plan.after.insert(end);
            }
        }
    }
    plan
}

fn plan_md031(d: &Doc) -> Plan {
    let mut plan = Plan::default();
    for i in 0..d.n() {
        if !d.fence_lines[i] {
            continue;
        }
        if i > 0 && !d.fence_lines[i - 1] && !d.lines[i - 1].trim().is_empty() && !d.in_code(i - 1)
        {
            plan.before.insert(i);
        }
        if d.fence_opens[i] {
            continue;
        }
        if i + 1 < d.n() && !d.fence_lines[i + 1] && !d.lines[i + 1].trim().is_empty() {
            plan.after.insert(i);
        }
    }
    plan
}

fn plan_md032(d: &Doc) -> Plan {
    let mut plan = Plan::default();
    for i in 0..d.n() {
        if !d.list[i] {
            continue;
        }
        if i > 0 {
            let prev = &d.lines[i - 1];
            let continues = d.list[i - 1]
                || is_indented(prev)
                || prev.trim_end_matches(' ').ends_with('>')
                || d.fence[i - 1]
                || d.fence_lines[i - 1];
            if !continues && !prev.trim().is_empty() && !d.in_code(i - 1) {
                plan.before.insert(i);
            }
        }
        if i + 1 < d.n() {
            let next_line = &d.lines[i + 1];
            let next = next_line.trim();
            let follows = d.list[i + 1]
                || is_indented(next_line)
                || next.starts_with("- ")
                || next.starts_with("* ")
                || next.starts_with("> ")
                || d.fence_lines[i + 1]
                || d.fence[i + 1];
            if !follows && !next.is_empty() {
                plan.after.insert(i);
            }
        }
    }
    plan
}

fn plan_md058(d: &Doc) -> Plan {
    let mut plan = Plan::default();
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..d.n() {
        if !d.table[i] || seen.contains(&i) {
            continue;
        }
        if i > 0 && !d.table[i - 1] && !d.lines[i - 1].trim().is_empty() && !d.in_code(i - 1) {
            plan.before.insert(i);
        }
        let mut j = i;
        while j + 1 < d.n() && d.table[j + 1] {
            j += 1;
        }
        seen.insert(j);
        if j + 1 < d.n() && !d.is_fence_line(j + 1) && !d.lines[j + 1].trim().is_empty() {
            plan.after.insert(j);
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdlint::{compile, Config};

    fn fixed(src: &str) -> String {
        let set = compile(&Config::default()).unwrap();
        fix_source(&set, src)
    }

    #[test]
    fn trailing_spaces_and_missing_final_newline_are_fixed() {
        assert_eq!(fixed("text   \nmore"), "text\nmore\n");
    }

    #[test]
    fn a_heading_gets_blank_lines_around_it() {
        assert_eq!(fixed("para\n# Title\npara\n"), "para\n\n# Title\n\npara\n");
    }

    #[test]
    fn a_fix_pass_that_changes_nothing_is_idempotent() {
        let once = fixed("# Title\n\ntext\n");
        assert_eq!(fixed(&once), once);
    }
}
