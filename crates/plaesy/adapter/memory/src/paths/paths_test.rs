//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use std::path::Path;

#[test]
fn models_root_is_app_scoped() {
    let p = models_root().expect("platform cache directory");
    assert!(
        p.ends_with(Path::new(APP_DIR).join("models")),
        "{}",
        p.display()
    );
}

#[test]
fn data_root_is_app_scoped() {
    let p = data_root().expect("platform data directory");
    assert!(p.ends_with(APP_DIR), "{}", p.display());
}

#[test]
fn models_and_data_are_separate_roots() {
    let models = models_root().expect("cache dir");
    let data = data_root().expect("data dir");
    assert_ne!(models, data);
}
