use std::path::Path;

use super::*;

#[test]
fn body_round_trips_through_parse_options() {
    let options = vec!["ya".to_owned(), "tidak".to_owned()];
    let body = body_with_options(&options);
    assert!(body.starts_with("options:\n"));
    assert_eq!(parse_options(&body), options);
}

#[test]
fn parse_options_ignores_lines_that_are_not_numbered() {
    let body = "options:\n1. satu\nbukan opsi\n2. dua\n";
    assert_eq!(
        parse_options(body),
        vec!["satu".to_owned(), "dua".to_owned()]
    );
}

#[test]
fn answered_text_flips_status_and_records_the_choice() {
    let note = "---\nid: \"d1\"\nstatus: \"proposed\"\n---\n\n# Q?\n\noptions:\n1. ya\n2. tidak\n";
    let answered = answered_text(note, "tidak").expect("proposed note");
    assert!(answered.contains("status: \"active\""));
    assert!(!answered.contains("status: \"proposed\""));
    assert!(answered.ends_with("\nchosen: tidak\n"));
}

#[test]
fn answered_text_refuses_a_note_that_is_not_proposed() {
    let note = "---\nstatus: \"active\"\n---\n";
    assert_eq!(answered_text(note, "ya"), None);
}

#[test]
fn parse_listing_reads_the_memory_json_shape() {
    let raw = r#"[
      {"id": "decision-1", "title": "Q?", "path": "decisions/q.md",
       "note_type": "decision", "status": "proposed", "project": null,
       "scope": [], "supersedes": [], "updated": "2026-11-24"}
    ]"#;
    let items = parse_listing(raw).expect("listing parses");
    assert_eq!(
        items,
        vec![Listed {
            id: "decision-1".to_owned(),
            title: "Q?".to_owned(),
            path: "decisions/q.md".to_owned(),
            status: "proposed".to_owned(),
        }]
    );
}

#[test]
fn parse_listing_rejects_non_json() {
    assert!(parse_listing("bukan json").is_err());
}

#[test]
fn parse_ask_needs_a_question_and_at_least_two_options() {
    assert_eq!(
        parse_ask("Pakai SQLite? | ya | tidak"),
        Ok((
            "Pakai SQLite?".to_owned(),
            vec!["ya".to_owned(), "tidak".to_owned()]
        ))
    );
    assert_eq!(parse_ask("Q? | satu"), Err(Usage::Ask));
    assert_eq!(parse_ask("Q? |  | b"), Err(Usage::Ask));
    assert_eq!(parse_ask(" | a | b"), Err(Usage::Ask));
}

#[test]
fn parse_pick_reads_an_id_and_a_positive_option_number() {
    assert_eq!(parse_pick("decision-1 2"), Ok(("decision-1".to_owned(), 2)));
    assert_eq!(
        parse_pick("  decision-1   2 "),
        Ok(("decision-1".to_owned(), 2))
    );
    assert_eq!(parse_pick("decision-1"), Err(Usage::Pick));
    assert_eq!(parse_pick("decision-1 0"), Err(Usage::Pick));
    assert_eq!(parse_pick("decision-1 x"), Err(Usage::Pick));
    assert_eq!(parse_pick("decision-1 2 3"), Err(Usage::Pick));
}

#[test]
fn ask_args_create_a_proposed_decision_in_the_decisions_folder() {
    let options = vec!["ya".to_owned(), "tidak".to_owned()];
    let args = ask_args(
        "Q?",
        &options,
        "decision-9",
        Some(Path::new("C:/vault")),
        Some("p1"),
    );
    assert_eq!(&args[..3], ["memory", "--vault", "C:/vault"]);
    assert!(args.windows(2).any(|w| w == ["--status", "proposed"]));
    assert!(args.windows(2).any(|w| w == ["--folder", "decisions"]));
    assert!(args.windows(2).any(|w| w == ["--id", "decision-9"]));
    assert!(args.windows(2).any(|w| w == ["--project", "p1"]));
    assert!(args.iter().any(|a| a.contains("1. ya")));
}

#[test]
fn list_args_filter_proposed_and_optionally_by_project() {
    let args = list_args(None, None);
    assert_eq!(args[0], "memory");
    assert!(args.windows(2).any(|w| w == ["--status", "proposed"]));
    assert!(args.windows(2).any(|w| w == ["--format", "json"]));
    assert!(!args.contains(&"--project".to_owned()));

    let scoped = list_args(None, Some("p1"));
    assert!(scoped.windows(2).any(|w| w == ["--project", "p1"]));
}

#[test]
fn update_args_pass_the_fingerprint_and_path() {
    let args = update_args(&"a".repeat(64), "decisions/q.md", None);
    assert!(args
        .windows(2)
        .any(|w| w == ["--if-match", &"a".repeat(64)]));
    assert!(args.windows(2).any(|w| w == ["--reason", "clarification"]));
    assert_eq!(args.last().map(String::as_str), Some("decisions/q.md"));
}

#[test]
fn memory_store_refuses_to_read_options_without_a_vault() {
    let store = MemoryPending::default();
    assert!(
        store.list().is_err(),
        "listing needs the vault to read option text"
    );
    assert!(store.answer("decision-1", 1).is_err());
}

#[test]
fn route_takes_only_reversible_and_confident_choices() {
    assert_eq!(route(true, 0.9, 0.8), Route::Take);
    assert_eq!(route(true, 0.8, 0.8), Route::Take, "threshold is inclusive");
    assert_eq!(
        route(false, 0.99, 0.8),
        Route::Ask,
        "irreversible always asks"
    );
    assert_eq!(route(true, 0.5, 0.8), Route::Ask, "low confidence asks");
    assert_eq!(
        route(true, f64::NAN, 0.8),
        Route::Ask,
        "NaN never takes a choice"
    );
}

#[test]
fn rewrite_decision_changes_title_heading_and_statement_only() {
    let note = "---\ntitle: \"Lama\"\nstatus: \"active\"\nid: \"decision-1\"\n---\n\n# Lama\n\nstatement: Lama\n";
    let out = rewrite_decision(note, "Baru \"quoted\"");
    assert!(
        out.contains("title: \"Baru \\\"quoted\\\"\""),
        "output was: {out}"
    );
    assert!(out.contains("# Baru \"quoted\""));
    assert!(out.contains("statement: Baru \"quoted\""));
    assert!(out.contains("status: \"active\""), "status must survive");
    assert!(out.contains("id: \"decision-1\""), "id must survive");
}
