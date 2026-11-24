//! Unit tests for `tests` split out of `delegate.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn only_the_claude_cli_starts_enabled_and_no_provider_is_billable_except_anthropic() {
    let cfg = DelegateConfig::defaults();
    let enabled: Vec<_> = cfg
        .providers
        .iter()
        .filter(|p| p.enabled)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(enabled, ["claude-cli"]);
    let billable: Vec<_> = cfg
        .providers
        .iter()
        .filter(|p| p.billable)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(billable, ["anthropic"]);
    let codex = cfg
        .providers
        .iter()
        .find(|p| p.name == "codex-cli")
        .unwrap();
    assert!(!codex.enabled);
    assert_eq!(codex.command, "codex");
    assert_eq!(codex.args, ["exec", "--ephemeral", "-"]);
}

#[test]
fn type_field_round_trips() {
    let p = DelegateProviderConfig {
        kind: "cli".into(),
        ..Default::default()
    };
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains(r#""type":"cli""#), "{json}");
    let back: DelegateProviderConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
}
