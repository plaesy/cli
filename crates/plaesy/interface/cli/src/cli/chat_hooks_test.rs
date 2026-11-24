use super::*;

fn hook(
    event: &str,
    command: &str,
    args: &[&str],
    on_failure: OnFailure,
    scope: Scope,
) -> HookSpec {
    HookSpec {
        event: event.to_owned(),
        command: command.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        timeout: Duration::from_secs(5),
        on_failure,
        scope,
    }
}

/// A command that succeeds, and one that fails, without a shell in the hook itself.
#[cfg(windows)]
fn ok_cmd() -> (&'static str, Vec<&'static str>) {
    ("cmd", vec!["/C", "exit", "0"])
}
#[cfg(windows)]
fn fail_cmd() -> (&'static str, Vec<&'static str>) {
    ("cmd", vec!["/C", "exit", "3"])
}
#[cfg(windows)]
fn slow_cmd() -> (&'static str, Vec<&'static str>) {
    ("cmd", vec!["/C", "ping", "-n", "6", "127.0.0.1"])
}
#[cfg(not(windows))]
fn ok_cmd() -> (&'static str, Vec<&'static str>) {
    ("sh", vec!["-c", "exit 0"])
}
#[cfg(not(windows))]
fn fail_cmd() -> (&'static str, Vec<&'static str>) {
    ("sh", vec!["-c", "exit 3"])
}
#[cfg(not(windows))]
fn slow_cmd() -> (&'static str, Vec<&'static str>) {
    ("sh", vec!["-c", "sleep 6"])
}

#[test]
fn parse_accepts_a_valid_hook_and_defaults_to_warn() {
    let value = serde_json::json!({
        "hooks": [{ "event": "decision.saved", "command": "notify", "args": ["{id}"], "timeout": "5s" }]
    });
    let hooks = parse_hooks(&value, Scope::User).expect("valid");
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].on_failure, OnFailure::Warn);
    assert_eq!(hooks[0].timeout, Duration::from_secs(5));
}

#[test]
fn parse_rejects_unknown_events_bad_timeouts_and_misplaced_block() {
    let unknown = serde_json::json!({"hooks": [{"event": "nope", "command": "x"}]});
    assert!(parse_hooks(&unknown, Scope::User).is_err());

    let bad_timeout = serde_json::json!({"hooks": [{"event": "session.start", "command": "x", "timeout": "soon"}]});
    assert!(parse_hooks(&bad_timeout, Scope::User).is_err());

    let misplaced_block = serde_json::json!({"hooks": [{"event": "decision.saved", "command": "x", "on_failure": "block"}]});
    assert!(parse_hooks(&misplaced_block, Scope::User).is_err());

    let block_before_send = serde_json::json!({"hooks": [{"event": "message.before_send", "command": "x", "on_failure": "block"}]});
    assert_eq!(
        parse_hooks(&block_before_send, Scope::User).expect("ok")[0].on_failure,
        OnFailure::Block
    );
}

#[test]
fn missing_settings_have_no_hooks() {
    assert!(parse_hooks(&serde_json::Value::Null, Scope::Repo)
        .expect("ok")
        .is_empty());
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        read_settings(&dir.path().join("absent.json")),
        serde_json::Value::Null
    );
}

#[test]
fn fingerprint_changes_when_the_hook_changes() {
    let base = hook(
        "decision.saved",
        "notify",
        &["{id}"],
        OnFailure::Warn,
        Scope::Repo,
    );
    let same = base.clone();
    assert_eq!(fingerprint(&base), fingerprint(&same));

    let other_arg = hook(
        "decision.saved",
        "notify",
        &["{title}"],
        OnFailure::Warn,
        Scope::Repo,
    );
    assert_ne!(fingerprint(&base), fingerprint(&other_arg));

    let other_mode = hook(
        "decision.saved",
        "notify",
        &["{id}"],
        OnFailure::Block,
        Scope::Repo,
    );
    assert_ne!(fingerprint(&base), fingerprint(&other_mode));
}

#[test]
fn repo_hooks_need_approval_and_user_hooks_do_not() {
    let repo = hook("session.start", "notify", &[], OnFailure::Warn, Scope::Repo);
    let user = hook("session.start", "notify", &[], OnFailure::Warn, Scope::User);
    assert!(!is_trusted(&repo, &[]));
    assert!(is_trusted(&repo, &[fingerprint(&repo)]));
    assert!(is_trusted(&user, &[]));
}

#[test]
fn expand_replaces_placeholders_in_one_pass() {
    let mut data = BTreeMap::new();
    data.insert("id".to_owned(), "{title}".to_owned());
    data.insert("title".to_owned(), "x".to_owned());
    let out = expand_args(
        &[
            "id={id}".to_owned(),
            "{unknown}".to_owned(),
            "tanpa".to_owned(),
        ],
        &data,
    );
    assert_eq!(
        out,
        vec!["id={title}", "{unknown}", "tanpa"],
        "a value is not expanded again"
    );
}

#[test]
fn run_succeeds_and_reports_failure_and_timeout() {
    let (cmd, args) = ok_cmd();
    assert!(run(
        &hook("session.start", cmd, &args, OnFailure::Warn, Scope::User),
        &BTreeMap::new()
    )
    .is_ok());

    let (cmd, args) = fail_cmd();
    assert!(run(
        &hook("session.start", cmd, &args, OnFailure::Warn, Scope::User),
        &BTreeMap::new()
    )
    .is_err());

    let (cmd, args) = slow_cmd();
    let mut slow = hook("session.start", cmd, &args, OnFailure::Warn, Scope::User);
    slow.timeout = Duration::from_millis(300);
    let err = run(&slow, &BTreeMap::new()).expect_err("must time out");
    assert!(err.contains("timed out"), "error was: {err}");
}

#[test]
fn warn_hook_failure_is_a_warning_and_block_hook_stops_the_send() {
    let (cmd, args) = fail_cmd();
    let warn = hook(
        "message.before_send",
        cmd,
        &args,
        OnFailure::Warn,
        Scope::User,
    );
    let out = fire(
        std::slice::from_ref(&warn),
        &[],
        "message.before_send",
        &BTreeMap::new(),
    );
    assert_eq!(out.warnings.len(), 1);
    assert!(out.blocked.is_none());

    let block = hook(
        "message.before_send",
        cmd,
        &args,
        OnFailure::Block,
        Scope::User,
    );
    let out = fire(
        std::slice::from_ref(&block),
        &[],
        "message.before_send",
        &BTreeMap::new(),
    );
    assert!(out.blocked.is_some());
}

#[test]
fn untrusted_repo_hook_is_skipped_and_its_fingerprint_is_reported() {
    let (cmd, args) = ok_cmd();
    let repo = hook("session.start", cmd, &args, OnFailure::Warn, Scope::Repo);
    let out = fire(
        std::slice::from_ref(&repo),
        &[],
        "session.start",
        &BTreeMap::new(),
    );
    assert_eq!(out.warnings.len(), 1);
    assert!(out.warnings[0].contains(&fingerprint(&repo)));
}

#[test]
fn fire_only_runs_hooks_of_the_named_event() {
    let (cmd, args) = fail_cmd();
    let other = hook("session.end", cmd, &args, OnFailure::Block, Scope::User);
    let out = fire(
        std::slice::from_ref(&other),
        &[],
        "session.start",
        &BTreeMap::new(),
    );
    assert_eq!(out, Outcome::default());
}
