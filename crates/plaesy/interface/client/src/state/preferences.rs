//! Preferensi tampilan: tema, bahasa, dan kepadatan layout. Perubahan lewat [`reduce`].

/// Tema tampilan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

/// Kepadatan layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    Compact,
    #[default]
    Comfortable,
}

/// Preferensi yang tersimpan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preferences {
    pub theme: Theme,
    pub locale: String,
    pub layout: Layout,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            locale: "id-ID".to_owned(),
            layout: Layout::default(),
        }
    }
}

/// Aksi yang mengubah preferensi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    SetTheme(Theme),
    SetLocale(String),
    SetLayout(Layout),
    Reset,
}

/// Galat: nilai preferensi tidak valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreferenceError {
    /// Locale harus berupa tag sederhana: huruf, angka, atau tanda hubung, 2 sampai 35 karakter.
    InvalidLocale(String),
}

/// Menerapkan `action` pada `state`. Mengembalikan keadaan baru, atau galat tanpa mengubah apa pun.
///
/// # Errors
///
/// [`PreferenceError::InvalidLocale`] bila locale tidak valid.
pub fn reduce(state: &Preferences, action: Action) -> Result<Preferences, PreferenceError> {
    let mut next = state.clone();
    match action {
        Action::SetTheme(theme) => next.theme = theme,
        Action::SetLocale(locale) => {
            if !is_valid_locale(&locale) {
                return Err(PreferenceError::InvalidLocale(locale));
            }
            next.locale = locale;
        }
        Action::SetLayout(layout) => next.layout = layout,
        Action::Reset => next = Preferences::default(),
    }
    Ok(next)
}

fn is_valid_locale(locale: &str) -> bool {
    let len = locale.chars().count();
    (2..=35).contains(&len)
        && locale
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[cfg(test)]
#[path = "preferences_test.rs"]
mod preferences_test;
