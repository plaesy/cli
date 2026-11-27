//! Renderer mobile (S11): scaffolding saja. Belum ada titik masuk, dependensi, atau build yang
//! diverifikasi untuk iOS atau Android. Toolchain Android dan iOS belum ada di mesin pengembangan.
//!
//! Rancangan ada di `.plaesy/plan/plaesy-client-parity/desktop-mobile.md`:
//! - Peluncuran: Dioxus mobile (fitur `dioxus/mobile`), setelah dukungannya di Dioxus 0.7 diverifikasi.
//! - Token: disimpan di keystore platform. API-nya belum diputuskan.
//! - HTTP native: sinkron seperti desktop, dengan batas waktu yang sama.
