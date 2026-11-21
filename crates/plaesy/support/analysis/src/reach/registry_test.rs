//! Unit tests for `tests` split out of `registry.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::reach::config::default_config;

#[test]
fn every_platform_is_registered() {
    let registry = Registry::new(default_config(), RegistryOptions::default());
    assert_eq!(registry.stats().platforms, Platform::ALL.len());
    assert!(registry.stats().initialized);
}

#[test]
fn a_platform_without_credentials_reports_its_health() {
    let registry = Registry::new(default_config(), RegistryOptions::default());
    let health = registry.health(Platform::Rss).expect("rss is registered");
    assert!(health.iter().any(|h| h.backend == "builtin" && h.available));
}
