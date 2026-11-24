//! Unit tests for `fuzz` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use proptest::prelude::*;
use serde_json::json;

proptest! {
    #[test]
    fn request_limit_integers_are_exactly_the_valid_range(raw in i64::MIN..=i64::MAX) {
        let params = json!({ "limit": raw });
        let result = request_limit(&params, 10);
        let should_be_ok = (1..=1000).contains(&raw);
        prop_assert_eq!(result.is_ok(), should_be_ok);
    }

    #[test]
    fn request_limit_floats_are_rejected_unless_whole_and_in_range(raw in -2000.0f64..2000.0f64) {
        let params = json!({ "limit": raw });
        let result = request_limit(&params, 10);
        let is_whole_in_range = raw.fract() == 0.0 && (1.0..=1000.0).contains(&raw);
        prop_assert_eq!(result.is_ok(), is_whole_in_range);
    }

    #[test]
    fn request_limit_strings_are_always_rejected(raw in ".*") {
        let params = json!({ "limit": raw });
        prop_assert!(request_limit(&params, 10).is_err());
    }

    #[test]
    fn request_limit_bools_are_always_rejected(raw in proptest::bool::ANY) {
        let params = json!({ "limit": raw });
        prop_assert!(request_limit(&params, 10).is_err());
    }

    #[test]
    fn request_limit_missing_key_uses_default(default in 1u64..=1000u64) {
        let params = json!({});
        match request_limit(&params, default) {
            Ok(v) => prop_assert_eq!(v, default as usize),
            Err(_) => prop_assert!(false, "missing limit must use the default, not error"),
        }
    }

    #[test]
    fn request_limit_arbitrary_json_never_panics(raw in prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<f64>().prop_map(|f| json!(f)),
        ".*".prop_map(Value::String),
        Just(json!([])),
        Just(json!({})),
    ]) {
        let params = json!({ "limit": raw });
        let _ = request_limit(&params, 10);
    }
}
