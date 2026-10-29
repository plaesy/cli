//! The adapter contract every platform implements, and the option helpers adapters share.

use std::collections::BTreeMap;

use super::exec::Deadline;
use super::types::{BackendHealth, BackendResult, Options, Platform};
use super::ReachError;

/// One platform: its backends, and how to run a query on each.
pub trait PlatformAdapter: Send + Sync {
    fn name(&self) -> Platform;

    /// Backends in preference order: the primary first, then fallbacks.
    fn backends(&self) -> &'static [&'static str];

    fn requires_auth(&self) -> bool {
        self.name().requires_auth()
    }

    /// Runs `query` on `backend`. A failed command is an `Ok` result with `success: false`.
    ///
    /// # Errors
    ///
    /// `backend` is not one of this adapter's backends, or `opts` asks for an unknown action.
    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError>;

    /// # Errors
    ///
    /// `backend` is not one of this adapter's backends.
    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError>;

    /// Applies credentials. The default ignores them.
    ///
    /// # Errors
    ///
    /// The credentials cannot be applied.
    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        let _ = creds;
        Ok(())
    }
}

/// The unknown-backend error for `platform`.
#[must_use]
pub fn unknown_backend(backend: &str, platform: Platform) -> ReachError {
    ReachError::UnknownBackend {
        backend: backend.to_owned(),
        platform,
    }
}

/// The value of `key`, or `""` when it is not set.
#[must_use]
pub fn opt<'a>(opts: &'a Options, key: &str) -> &'a str {
    opts.get(key).map_or("", String::as_str)
}

/// The value of `key`, or `default` when it is not set or empty.
#[must_use]
pub fn opt_or<'a>(opts: &'a Options, key: &str, default: &'a str) -> &'a str {
    match opts.get(key).map(String::as_str) {
        Some(v) if !v.is_empty() => v,
        _ => default,
    }
}

/// `url` with `https://` in front when it does not start with `http`.
#[must_use]
pub fn with_scheme(url: &str) -> String {
    if url.starts_with("http") {
        url.to_owned()
    } else {
        format!("https://{url}")
    }
}

/// Appends a flag and its value when the value is non-empty.
pub fn push_flag(args: &mut Vec<String>, flag: &str, value: &str) {
    if !value.is_empty() {
        args.push(flag.to_owned());
        args.push(value.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_scheme_adds_https_only_when_missing() {
        assert_eq!(with_scheme("example.com"), "https://example.com");
        assert_eq!(with_scheme("https://example.com"), "https://example.com");
    }

    #[test]
    fn opt_or_treats_an_empty_value_as_missing() {
        let mut opts = Options::new();
        opts.insert("action".into(), String::new());
        assert_eq!(opt_or(&opts, "action", "search"), "search");
        assert_eq!(opt(&opts, "absent"), "");
    }
}
