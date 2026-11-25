use serde_json::Value;

use super::super::chat_text::Lang;
use super::{apply, hint, parse, render, Pick};
use crate::config::delegate::DelegateProviderConfig;

fn entry(name: &str, enabled: bool, model: &str) -> DelegateProviderConfig {
    DelegateProviderConfig {
        name: name.into(),
        kind: "ollama".into(),
        enabled,
        model: model.into(),
        ..Default::default()
    }
}

fn providers() -> Vec<DelegateProviderConfig> {
    vec![
        entry("alpha", true, "m1"),
        entry("beta", false, "m2"),
        entry("gamma", true, ""),
    ]
}

#[test]
fn render_numbers_every_provider_and_marks_the_active_and_disabled_ones() {
    let text = render(&providers(), "alpha");
    assert!(text.contains("* 1. alpha  m1"), "{text}");
    assert!(text.contains("  2. beta  m2  (nonaktif)"), "{text}");
    assert!(text.contains("  3. gamma  -"), "{text}");
}

#[test]
fn hint_is_bilingual() {
    assert!(hint(Lang::En).contains("/model <number>"));
    assert!(hint(Lang::Id).contains("pilih dengan"));
}

#[test]
fn parse_accepts_a_number_a_name_and_a_name_with_model() {
    let list = providers();
    assert_eq!(parse("1", &list, "alpha").unwrap().target(), "alpha");
    assert_eq!(parse("gamma", &list, "alpha").unwrap().target(), "gamma");
    assert_eq!(
        parse("alpha@m9", &list, "gamma").unwrap().target(),
        "alpha@m9"
    );
}

#[test]
fn a_bare_unknown_word_is_a_model_for_the_active_provider() {
    let list = providers();
    let pick = parse("llama3.2", &list, "gamma").unwrap();
    assert_eq!(pick.provider, "gamma");
    assert_eq!(pick.model.as_deref(), Some("llama3.2"));
}

#[test]
fn parse_refuses_zero_out_of_range_disabled_and_unknown_names() {
    let list = providers();
    assert!(parse("0", &list, "alpha").is_err());
    assert!(parse("9", &list, "alpha").is_err());
    assert!(parse("2", &list, "alpha").unwrap_err().contains("nonaktif"));
    assert!(parse("beta@m3", &list, "alpha")
        .unwrap_err()
        .contains("nonaktif"));
    assert!(parse("@m3", &list, "alpha").is_err());
}

#[test]
fn apply_moves_the_pick_to_the_front_and_sets_its_model() {
    let settings = r#"{"telemetry":{"enabled":false},"delegate":{"chains":{"main":["alpha"]}}}"#;
    let pick = Pick {
        provider: "gamma".into(),
        model: Some("big".into()),
    };
    let out = apply(settings, &providers(), &pick).unwrap();
    let root: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(root["telemetry"]["enabled"], Value::Bool(false));
    assert_eq!(root["delegate"]["chains"]["main"][0], "alpha");
    assert_eq!(root["delegate"]["providers"][0]["name"], "gamma");
    assert_eq!(root["delegate"]["providers"][0]["model"], "big");
    assert_eq!(root["delegate"]["providers"][1]["name"], "alpha");
}

#[test]
fn apply_without_a_model_keeps_the_provider_model() {
    let pick = Pick {
        provider: "alpha".into(),
        model: None,
    };
    let out = apply("", &providers(), &pick).unwrap();
    let root: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(root["delegate"]["providers"][0]["name"], "alpha");
    assert_eq!(root["delegate"]["providers"][0]["model"], "m1");
}

#[test]
fn apply_refuses_invalid_settings_and_unknown_providers() {
    let pick = Pick {
        provider: "alpha".into(),
        model: None,
    };
    assert!(apply("{not json", &providers(), &pick).is_err());
    assert!(apply("[]", &providers(), &pick).is_err());
    let missing = Pick {
        provider: "zeta".into(),
        model: None,
    };
    assert!(apply("", &providers(), &missing)
        .unwrap_err()
        .contains("zeta"));
}
