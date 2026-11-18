//! Unit tests for `tests` split out of `inflight.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
