use super::*;
use crate::error::PluginError;

#[test]
fn default_is_valid_and_maps_to_pages() {
    let defaults = ResourceLimits::default();
    assert!(defaults.validate().is_ok());
    assert_eq!(defaults.memory_pages(), 1024);
}

#[test]
fn rejects_zero_or_excessive_fuel() {
    assert!(matches!(
        ResourceLimits::new(0, DEFAULT_MEMORY_BYTES),
        Err(PluginError::InvalidLimits(_))
    ));
    assert!(matches!(
        ResourceLimits::new(MAX_FUEL + 1, DEFAULT_MEMORY_BYTES),
        Err(PluginError::InvalidLimits(_))
    ));
    assert!(ResourceLimits::new(MAX_FUEL, DEFAULT_MEMORY_BYTES).is_ok());
}

#[test]
fn rejects_memory_out_of_range_or_not_page_aligned() {
    assert!(ResourceLimits::new(1, MIN_MEMORY_BYTES - WASM_PAGE_BYTES).is_err());
    assert!(ResourceLimits::new(1, MAX_MEMORY_BYTES + WASM_PAGE_BYTES).is_err());
    assert!(ResourceLimits::new(1, MIN_MEMORY_BYTES + 1).is_err());
    assert!(ResourceLimits::new(1, MIN_MEMORY_BYTES).is_ok());
    assert!(ResourceLimits::new(1, MAX_MEMORY_BYTES).is_ok());
}

#[test]
fn deserializes_config_and_rejects_unknown_keys() {
    let parsed: ResourceLimits =
        serde_json::from_str(r#"{"fuel":5000,"memory_bytes":1048576}"#).unwrap();
    assert_eq!(
        parsed,
        ResourceLimits {
            fuel: 5000,
            memory_bytes: 1_048_576,
        }
    );
    assert!(parsed.validate().is_ok());
    assert!(serde_json::from_str::<ResourceLimits>(
        r#"{"fuel":1,"memory_bytes":1048576,"extra":1}"#
    )
    .is_err());
}
