//! Unit tests for `tests` split out of `web.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn ten_seconds() -> Deadline {
    Deadline::after(Duration::from_secs(10))
}

#[test]
fn an_unknown_backend_is_an_error() {
    assert!(WebAdapter
        .execute("nope", "example.com", &Options::new(), ten_seconds())
        .is_err());
}

#[test]
fn youtube_search_terms_use_ytsearch() {
    let args = YouTubeAdapter::yt_dlp_args("rust tutorial", &Options::new());
    assert!(args.iter().any(|a| a == "ytsearch:rust tutorial"));
}

#[test]
fn youtube_urls_are_fetched_as_given() {
    let args = YouTubeAdapter::yt_dlp_args("https://youtu.be/abc", &Options::new());
    assert!(args.iter().any(|a| a == "https://youtu.be/abc"));
    assert!(!args.iter().any(|a| a.starts_with("ytsearch:")));
}
