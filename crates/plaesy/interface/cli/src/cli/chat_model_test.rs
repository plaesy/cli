use super::super::chat_text::Lang;
use super::{Pick, apply, from_legacy_json, hint, parse, render, save};
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

fn first_name(table: &toml::Table) -> Option<&str> {
    table["delegate"]["providers"][0]["name"].as_str()
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
    assert!(
        parse("beta@m3", &list, "alpha")
            .unwrap_err()
            .contains("nonaktif")
    );
    assert!(parse("@m3", &list, "alpha").is_err());
}

#[test]
fn apply_moves_the_pick_to_the_front_and_sets_its_model() {
    let config = "[telemetry]\nenabled = false\n\n[delegate.chains]\nmain = [\"alpha\"]\n";
    let pick = Pick {
        provider: "gamma".into(),
        model: Some("big".into()),
    };
    let out = apply(config, &providers(), &pick).unwrap();
    let root: toml::Table = out.parse().unwrap();
    assert_eq!(root["telemetry"]["enabled"].as_bool(), Some(false));
    assert_eq!(root["delegate"]["chains"]["main"][0].as_str(), Some("alpha"));
    assert_eq!(first_name(&root), Some("gamma"));
    assert_eq!(root["delegate"]["providers"][0]["model"].as_str(), Some("big"));
    assert_eq!(root["delegate"]["providers"][1]["name"].as_str(), Some("alpha"));
}

#[test]
fn apply_without_a_model_keeps_the_provider_model() {
    let pick = Pick {
        provider: "alpha".into(),
        model: None,
    };
    let out = apply("", &providers(), &pick).unwrap();
    let root: toml::Table = out.parse().unwrap();
    assert_eq!(first_name(&root), Some("alpha"));
    assert_eq!(root["delegate"]["providers"][0]["model"].as_str(), Some("m1"));
}

#[test]
fn apply_refuses_invalid_config_and_unknown_providers() {
    let pick = Pick {
        provider: "alpha".into(),
        model: None,
    };
    assert!(apply("{not toml", &providers(), &pick).is_err());
    assert!(apply("delegate = 3", &providers(), &pick).is_err());
    let missing = Pick {
        provider: "zeta".into(),
        model: None,
    };
    assert!(
        apply("", &providers(), &missing)
            .unwrap_err()
            .contains("zeta")
    );
}

#[test]
fn a_legacy_settings_file_is_converted_and_its_null_is_refused() {
    let converted = from_legacy_json(r#"{"telemetry":{"enabled":false}}"#).unwrap();
    let root: toml::Table = converted.parse().unwrap();
    assert_eq!(root["telemetry"]["enabled"].as_bool(), Some(false));
    assert!(from_legacy_json(r#"{"a": null}"#).is_err());
}

#[test]
fn the_first_save_copies_settings_json_into_config_toml_and_leaves_it_alone() {
    let dir = tempfile::tempdir().unwrap();
    let plaesy = dir.path().join(".plaesy");
    std::fs::create_dir_all(&plaesy).unwrap();
    let legacy = r#"{"telemetry":{"enabled":false},"delegate":{"chains":{"main":["alpha"]}}}"#;
    std::fs::write(plaesy.join("settings.json"), legacy).unwrap();

    let pick = Pick {
        provider: "gamma".into(),
        model: None,
    };
    save(dir.path(), &providers(), &pick).unwrap();

    let written: toml::Table = std::fs::read_to_string(plaesy.join("config.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(written["telemetry"]["enabled"].as_bool(), Some(false));
    assert_eq!(first_name(&written), Some("gamma"));
    assert_eq!(
        std::fs::read_to_string(plaesy.join("settings.json")).unwrap(),
        legacy
    );
}
