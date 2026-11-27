//! Renderer desktop (Dioxus, S10). Membaca pengaturan dari berkas, memasangkan perangkat, dan
//! menyimpan token di keychain OS. Token tidak pernah masuk ke berkas, URL, atau log.
//!
//! Editor flow belum dipasang di sini. App editor ada di `platform/web` dan baru bisa dipakai
//! bersama setelah dipindah ke `ui/` (langkah berikutnya, menyentuh berkas web).

mod pairing;
mod token;

use crate::state::config::{format_settings, parse_settings, validate_settings, ClientSettings};
use dioxus::prelude::*;
use std::path::PathBuf;

pub fn run() {
    dioxus::launch(App);
}

/// Lokasi berkas pengaturan. `PLAESY_DESKTOP_CONFIG` menimpa lokasi bawaan.
fn settings_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PLAESY_DESKTOP_CONFIG") {
        return Some(PathBuf::from(path));
    }
    let root = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(root.join("plaesy").join("client.json"))
}

fn load_settings() -> ClientSettings {
    settings_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| parse_settings(&text).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &ClientSettings) -> Result<(), String> {
    let path = settings_path().ok_or("folder konfigurasi tidak ditemukan")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&path, format_settings(settings)).map_err(|error| error.to_string())
}

#[component]
fn App() -> Element {
    let initial = use_hook(load_settings);
    let mut base_url = use_signal(|| initial.base_url.clone());
    let mut project = use_signal(|| initial.project.clone());
    let mut device = use_signal(|| initial.device_name.clone());
    let mut code = use_signal(String::new);
    let mut status = use_signal(|| "Belum dipasangkan".to_owned());

    rsx! {
        main { class: "desktop-main",
            h1 { "Plaesy desktop" }
            label { class: "field-label", "for": "desktop-base", "Alamat server" }
            input { id: "desktop-base", class: "field-input", value: "{base_url()}",
                oninput: move |event| base_url.set(event.value()),
            }
            label { class: "field-label", "for": "desktop-project", "Project" }
            input { id: "desktop-project", class: "field-input", placeholder: "prj_...", value: "{project()}",
                oninput: move |event| project.set(event.value()),
            }
            label { class: "field-label", "for": "desktop-device", "Nama perangkat" }
            input { id: "desktop-device", class: "field-input", value: "{device()}",
                oninput: move |event| device.set(event.value()),
            }
            label { class: "field-label", "for": "desktop-code", "Kode pairing" }
            input { id: "desktop-code", class: "field-input", placeholder: "PAIR-...", value: "{code()}",
                oninput: move |event| code.set(event.value()),
            }
            button { class: "button",
                onclick: move |_| {
                    let settings = ClientSettings {
                        base_url: base_url(),
                        project: project(),
                        device_name: device(),
                    };
                    if let Err(message) = validate_settings(&settings) {
                        status.set(message);
                        return;
                    }
                    let pairing_code = code();
                    status.set("Memasangkan…".to_owned());
                    spawn(async move {
                        match pairing::exchange(&settings.base_url, &pairing_code) {
                            Ok(pairing) => {
                                let account = token::account_for(&settings.base_url, &settings.project);
                                match token::save(&account, &pairing.token) {
                                    Ok(()) => match save_settings(&settings) {
                                        Ok(()) => status.set(format!(
                                            "Terpasang, berlaku sampai {}",
                                            pairing.expires_at
                                        )),
                                        Err(error) => status.set(format!(
                                            "Token tersimpan, pengaturan gagal: {error}"
                                        )),
                                    },
                                    Err(error) => status.set(format!("Gagal menyimpan token: {error}")),
                                }
                            }
                            Err(error) => status.set(error.to_string()),
                        }
                    });
                },
                "Pasangkan"
            }
            p { class: "status", "{status()}" }
        }
    }
}
