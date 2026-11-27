//! Titik masuk per platform (ADR 0011). Platform `web` dan `desktop` sudah ada; `mobile` baru
//! scaffolding.

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "mobile")]
pub mod mobile;
#[cfg(feature = "web")]
pub mod web;
