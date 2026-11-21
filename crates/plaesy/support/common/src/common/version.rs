//! Release version handling.

/// Fallback used when no valid version is available.
pub const FALLBACK_VERSION: &str = "0.0.0";

/// Returns `raw` trimmed when it is strict `MAJOR.MINOR.PATCH` semver,
/// otherwise [`FALLBACK_VERSION`].
///
/// # Examples
///
/// ```
/// assert_eq!(plaesy_common::common::version::normalize("1.2.3\n"), "1.2.3");
/// assert_eq!(plaesy_common::common::version::normalize("v1.2"), "0.0.0");
/// ```
#[must_use]
pub fn normalize(raw: &str) -> &str {
    let candidate = raw.trim();
    if is_strict_semver(candidate) {
        candidate
    } else {
        FALLBACK_VERSION
    }
}

fn is_strict_semver(s: &str) -> bool {
    let mut parts = s.split('.');
    let ok = (0..3).all(|_| {
        parts
            .next()
            .is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    });
    ok && parts.next().is_none()
}

#[cfg(test)]
#[path = "version_test.rs"]
mod version_test;
