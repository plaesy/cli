//! Titik masuk per platform (ADR 0011). Platform `web` dan `desktop` sudah ada.

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "web")]
pub mod web;
