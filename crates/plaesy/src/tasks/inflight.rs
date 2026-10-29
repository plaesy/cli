//! The In-Flight Tasks section of `.plaesy/context.md`.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::{Status, TaskRef, TasksError};

/// Heading of the section that lists active tasks.
pub const IN_FLIGHT_HEADING: &str = "## In-Flight Tasks";

/// Renders one line per task in `todo` or `doing`, in the order given.
#[must_use]
pub fn render_in_flight(refs: &[TaskRef]) -> String {
    let mut out = String::new();
    for r in refs
        .iter()
        .filter(|r| matches!(r.status, Status::Todo | Status::Doing))
    {
        let _ = writeln!(out, "- [{}] {} ({}): {}", r.spec, r.name, r.status, r.title);
    }
    if out.is_empty() {
        "- none\n".to_owned()
    } else {
        out
    }
}

/// Replaces the body under `heading`, up to the next `## ` heading or the end of
/// the file. The heading line is kept. A missing heading is appended. Line endings
/// of `content` are preserved.
#[must_use]
pub fn replace_section(content: &str, heading: &str, body: &str) -> String {
    let eol = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let normalized = content.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let body_lines: Vec<&str> = body.trim_end_matches('\n').split('\n').collect();

    let Some(start) = lines
        .iter()
        .position(|line| line.trim_end_matches([' ', '\t']) == heading)
    else {
        let trimmed = normalized.trim_end_matches('\n');
        return format!("{trimmed}\n\n{heading}\n{}\n", body_lines.join("\n")).replace('\n', eol);
    };

    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |offset| start + 1 + offset);

    let mut out: Vec<&str> = Vec::new();
    out.extend_from_slice(&lines[..=start]);
    out.extend_from_slice(&body_lines);
    out.push("");
    out.extend_from_slice(&lines[end..]);
    let result = out.join("\n");
    result.trim_end_matches('\n').to_owned().replace('\n', eol) + eol
}

/// Rewrites the In-Flight section of `<project_root>/.plaesy/context.md`. Only
/// that section changes. Returns whether the file changed.
///
/// # Errors
///
/// I/O failures reading or writing the file.
pub fn sync_in_flight(project_root: &Path, refs: &[TaskRef]) -> Result<bool, TasksError> {
    let path = project_root.join(".plaesy").join("context.md");
    let current = fs::read_to_string(&path)
        .map_err(|e| TasksError::io(format!("reading {}", path.display()), e))?;
    let updated = replace_section(&current, IN_FLIGHT_HEADING, &render_in_flight(refs));
    if updated == current {
        return Ok(false);
    }
    fs::write(&path, &updated)
        .map_err(|e| TasksError::io(format!("writing {}", path.display()), e))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: Status, name: &str) -> TaskRef {
        TaskRef {
            spec: "auth".into(),
            status,
            name: name.into(),
            title: "T".into(),
            phase: String::new(),
        }
    }

    #[test]
    fn only_todo_and_doing_are_rendered() {
        let refs = [task(Status::Done, "a"), task(Status::Doing, "b")];
        assert_eq!(render_in_flight(&refs), "- [auth] b (doing): T\n");
        assert_eq!(render_in_flight(&[]), "- none\n");
    }

    #[test]
    fn replace_keeps_the_rest_of_the_file() {
        let content = "# Ctx\n\n## In-Flight Tasks\n- old\n\n## Next\nkeep me\n";
        let out = replace_section(content, IN_FLIGHT_HEADING, "- new\n");
        assert_eq!(
            out,
            "# Ctx\n\n## In-Flight Tasks\n- new\n\n## Next\nkeep me\n"
        );
    }

    #[test]
    fn missing_heading_is_appended() {
        let out = replace_section("# Ctx\n", IN_FLIGHT_HEADING, "- none\n");
        assert_eq!(out, "# Ctx\n\n## In-Flight Tasks\n- none\n");
    }

    #[test]
    fn crlf_endings_are_preserved() {
        let out = replace_section(
            "# Ctx\r\n## In-Flight Tasks\r\n- old\r\n",
            IN_FLIGHT_HEADING,
            "- new\n",
        );
        assert!(out.contains("\r\n"));
        assert!(!out.replace("\r\n", "").contains('\r'));
    }
}
