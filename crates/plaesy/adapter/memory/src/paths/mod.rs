//! Per-OS default locations for models and data.
//!
//! Each root follows the platform convention, resolved by the `dirs` crate:
//!
//! | OS      | Models (re-downloadable, cache)                | Data (database, datasets)                         |
//! |---------|------------------------------------------------|---------------------------------------------------|
//! | Linux   | `$XDG_CACHE_HOME/plaesy/models` (`~/.cache`)   | `$XDG_DATA_HOME/plaesy` (`~/.local/share`)         |
//! | macOS   | `~/Library/Caches/plaesy/models`               | `~/Library/Application Support/plaesy`            |
//! | Windows | `%LOCALAPPDATA%\plaesy\models`                 | `%LOCALAPPDATA%\plaesy`                           |
//!
//! Models are a cache: the system may delete them and `scripts/install-model.sh` re-downloads them.
//! Data is kept. The legacy `~/.plaesy-models/` directory stays as the last lookup fallback so that
//! existing installs keep working.

use std::path::PathBuf;

/// Application directory name under each platform root.
const APP_DIR: &str = "plaesy";

/// Root for re-downloadable model files: `<cache>/plaesy/models`.
pub fn models_root() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join(APP_DIR).join("models"))
}

/// Root for persistent data (database, datasets): `<data-local>/plaesy`.
pub fn data_root() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join(APP_DIR))
}

/// Legacy model root used before the per-OS layout: `~/.plaesy-models`.
///
/// Kept only as a lookup fallback. New installs never write here.
pub fn legacy_models_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".plaesy-models"))
}

#[cfg(test)]
#[path = "paths_test.rs"]
mod paths_test;
