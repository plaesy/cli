use super::*;

fn settings() -> ClientSettings {
    ClientSettings {
        base_url: "https://plaesy.example.com".to_owned(),
        project: "prj_abc123".to_owned(),
        device_name: "laptop kantor".to_owned(),
    }
}

#[test]
fn settings_round_trip_through_json() {
    let text = format_settings(&settings());
    assert_eq!(parse_settings(&text), Ok(settings()));
}

#[test]
fn missing_fields_fall_back_to_defaults() {
    let parsed = parse_settings(r#"{"project": "prj_x1"}"#).expect("JSON objek valid");
    assert_eq!(parsed.project, "prj_x1");
    assert_eq!(parsed.base_url, ClientSettings::default().base_url);
    assert_eq!(parsed.device_name, ClientSettings::default().device_name);
}

#[test]
fn saved_text_never_contains_a_token_field() {
    assert!(!format_settings(&settings()).contains("token"));
}

#[test]
fn non_json_and_non_object_are_refused() {
    assert!(parse_settings("bukan json").is_err());
    assert!(parse_settings("[1, 2]").is_err());
}

#[test]
fn valid_settings_pass_validation() {
    assert_eq!(validate_settings(&settings()), Ok(()));
}

#[test]
fn bad_base_url_project_or_device_name_is_refused() {
    let mut bad_url = settings();
    bad_url.base_url = "ftp://x".to_owned();
    assert!(validate_settings(&bad_url).is_err());

    let mut bad_project = settings();
    bad_project.project = "PRJ_X".to_owned();
    assert!(validate_settings(&bad_project).is_err());

    let mut blank_device = settings();
    blank_device.device_name = "   ".to_owned();
    assert!(validate_settings(&blank_device).is_err());
}
