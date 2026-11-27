//! Token per perangkat di penyimpanan rahasia OS (Windows Credential Manager di Windows). Token
//! tidak pernah ditulis ke berkas, URL, atau log. Galat keyring dikembalikan apa adanya tanpa
//! isi token.

use keyring::Entry;

const SERVICE: &str = "plaesy-client";

/// Kunci penyimpanan untuk satu pasangan server dan project.
#[must_use]
pub fn account_for(base_url: &str, project: &str) -> String {
    format!("{}|{}", base_url.trim_end_matches('/'), project)
}

fn entry(account: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, account).map_err(|error| error.to_string())
}

/// Menyimpan token untuk `account`, menggantikan token lama.
///
/// # Errors
///
/// Galat penyimpanan keychain.
pub fn save(account: &str, token: &str) -> Result<(), String> {
    entry(account)?
        .set_password(token)
        .map_err(|error| error.to_string())
}

/// Membaca token untuk `account`. `None` bila belum ada.
///
/// # Errors
///
/// Galat selain "tidak ada" dari keychain.
pub fn load(account: &str) -> Result<Option<String>, String> {
    match entry(account)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}
