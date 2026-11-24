use super::*;

#[test]
fn preferences_defaults_are_system_theme_indonesian_comfortable() {
    let prefs = Preferences::default();
    assert_eq!(prefs.theme, Theme::System);
    assert_eq!(prefs.locale, "id-ID");
    assert_eq!(prefs.layout, Layout::Comfortable);
}

#[test]
fn preferences_set_theme_and_layout_return_new_state() {
    let base = Preferences::default();
    let dark = reduce(&base, Action::SetTheme(Theme::Dark)).unwrap();
    let compact = reduce(&dark, Action::SetLayout(Layout::Compact)).unwrap();
    assert_eq!(compact.theme, Theme::Dark);
    assert_eq!(compact.layout, Layout::Compact);
    // Keadaan awal tidak berubah.
    assert_eq!(base.theme, Theme::System);
}

#[test]
fn preferences_valid_locale_is_accepted() {
    let next = reduce(
        &Preferences::default(),
        Action::SetLocale("en-US".to_owned()),
    )
    .unwrap();
    assert_eq!(next.locale, "en-US");
}

#[test]
fn preferences_invalid_locale_is_refused_without_changing_state() {
    let base = Preferences::default();
    let long = "a".repeat(36);
    let bad_locales: [&str; 5] = ["", "x", "en US", "en_US", long.as_str()];
    for bad in bad_locales {
        let result = reduce(&base, Action::SetLocale(bad.to_owned()));
        assert!(
            matches!(result, Err(PreferenceError::InvalidLocale(_))),
            "{bad:?}"
        );
    }
    assert_eq!(base.locale, "id-ID");
}

#[test]
fn preferences_reset_restores_defaults() {
    let changed = reduce(&Preferences::default(), Action::SetTheme(Theme::Light)).unwrap();
    assert_eq!(
        reduce(&changed, Action::Reset).unwrap(),
        Preferences::default()
    );
}
