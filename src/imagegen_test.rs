//! Unit tests for `tests` split out of `imagegen.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn provider_names_are_exact() {
    assert_eq!(Provider::parse("openai").unwrap(), Provider::OpenAi);
    assert_eq!(Provider::parse("gemini").unwrap(), Provider::Gemini);
    assert!(matches!(
        Provider::parse("midjourney"),
        Err(ImageError::UnknownProvider(_))
    ));
}

#[test]
fn missing_arguments_are_rejected_before_any_request() {
    assert!(matches!(
        generate(&Options::default()),
        Err(ImageError::MissingArgs)
    ));
}

#[test]
fn base64_payload_decodes() {
    assert_eq!(decode("aGVsbG8=").unwrap(), b"hello");
    assert!(decode("%%%").is_err());
}
