//! Adapters for open web sources that need no credentials: web pages, YouTube, GitHub, RSS.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::reach::adapter::{opt, opt_or, unknown_backend, with_scheme, PlatformAdapter};
use crate::reach::exec::{argv, capture, check_command, run_backend, Deadline};
use crate::reach::rss;
use crate::reach::types::{BackendHealth, BackendResult, Options, Platform};
use crate::reach::ReachError;

/// Reads web pages through Jina Reader, or directly with curl.
pub struct WebAdapter;

impl PlatformAdapter for WebAdapter {
    fn name(&self) -> Platform {
        Platform::Web
    }

    fn backends(&self) -> &'static [&'static str] {
        &["jina-reader", "curl"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        _opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        let url = with_scheme(query);
        match backend {
            "jina-reader" => {
                let jina = format!("https://r.jina.ai/{url}");
                Ok(run_backend(
                    Platform::Web,
                    backend,
                    &argv(&["curl", "-s", "-L", &jina]),
                    &[],
                    deadline,
                ))
            }
            "curl" => Ok(run_backend(
                Platform::Web,
                backend,
                &argv(&["curl", "-s", "-L", "-A", "Mozilla/5.0", &url]),
                &[],
                deadline,
            )),
            other => Err(unknown_backend(other, Platform::Web)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "jina-reader" | "curl" => Ok(check_command(
                Platform::Web,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Web)),
        }
    }
}

/// YouTube videos and transcripts through yt-dlp.
pub struct YouTubeAdapter;

impl YouTubeAdapter {
    /// The yt-dlp arguments. A URL is fetched as given; anything else is a search term.
    #[must_use]
    pub fn yt_dlp_args(query: &str, opts: &Options) -> Vec<String> {
        let mut args = argv(&["yt-dlp"]);
        let subtitles = opt(opts, "subtitles");
        if subtitles == "true" || subtitles == "1" {
            args.extend(argv(&[
                "--write-subs",
                "--sub-lang",
                "en,zh,auto",
                "--skip-download",
            ]));
        }
        if query.contains("youtube.com") || query.contains("youtu.be") {
            args.push(query.to_owned());
        } else {
            args.push(format!("ytsearch:{query}"));
        }
        args.extend(argv(&["-o", "-"]));
        args
    }
}

impl PlatformAdapter for YouTubeAdapter {
    fn name(&self) -> Platform {
        Platform::YouTube
    }

    fn backends(&self) -> &'static [&'static str] {
        &["yt-dlp", "youtube-transcript-api"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "yt-dlp" => Ok(run_backend(
                Platform::YouTube,
                backend,
                &Self::yt_dlp_args(query, opts),
                &[],
                deadline,
            )),
            // Not implemented in the Go version either: it falls back to yt-dlp with subtitles on.
            "youtube-transcript-api" => {
                let mut with_subs = Options::new();
                with_subs.insert("subtitles".into(), "true".into());
                Ok(run_backend(
                    Platform::YouTube,
                    backend,
                    &Self::yt_dlp_args(query, &with_subs),
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::YouTube)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "yt-dlp" => Ok(check_command(
                Platform::YouTube,
                backend,
                &["yt-dlp", "--version"],
            )),
            "youtube-transcript-api" => Ok(check_command(
                Platform::YouTube,
                backend,
                &["python3", "-c", "import youtube_transcript_api"],
            )),
            other => Err(unknown_backend(other, Platform::YouTube)),
        }
    }
}

/// GitHub repositories through the gh CLI, or git ls-remote.
pub struct GitHubAdapter;

impl PlatformAdapter for GitHubAdapter {
    fn name(&self) -> Platform {
        Platform::GitHub
    }

    fn backends(&self) -> &'static [&'static str] {
        &["gh-cli", "git"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "gh-cli" => {
                let action = opt_or(opts, "action", "view");
                let mut args = argv(&["gh", "repo", action, query]);
                let fields = match action {
                    "view" => "name,description,url,stargazerCount,forkCount,primaryLanguage,issues,latestRelease",
                    "list-issues" | "list-prs" => "number,title,state,labels,createdAt,updatedAt",
                    "list-releases" => "tagName,name,createdAt,isLatest,isPrerelease",
                    _ => "",
                };
                if !fields.is_empty() {
                    args.extend(argv(&["--json", fields]));
                }
                Ok(run_backend(Platform::GitHub, backend, &args, &[], deadline))
            }
            "git" => {
                let url = format!("https://github.com/{query}");
                Ok(run_backend(
                    Platform::GitHub,
                    backend,
                    &argv(&["git", "ls-remote", &url]),
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::GitHub)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "gh-cli" => Ok(check_command(
                Platform::GitHub,
                backend,
                &["gh", "--version"],
            )),
            "git" => Ok(check_command(
                Platform::GitHub,
                backend,
                &["git", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::GitHub)),
        }
    }

    /// Runs `gh auth setup-git` with `GH_TOKEN` set, when a token is given.
    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        let Some(token) = creds.get("token").filter(|t| !t.is_empty()) else {
            return Ok(());
        };
        let env = [("GH_TOKEN".to_owned(), token.clone())];
        let out = capture(
            &argv(&["gh", "auth", "setup-git"]),
            &env,
            None,
            Deadline::after(Duration::from_secs(30)),
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(ReachError::Io(std::io::Error::other(format!(
                "gh auth setup-git failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))))
        }
    }
}

/// RSS and Atom feeds, read by the built-in parser or by Python feedparser.
pub struct RssAdapter;

impl PlatformAdapter for RssAdapter {
    fn name(&self) -> Platform {
        Platform::Rss
    }

    fn backends(&self) -> &'static [&'static str] {
        &["builtin", "feedparser"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        _opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "builtin" => Ok(rss::fetch_builtin(query, deadline)),
            "feedparser" => Ok(run_backend(
                Platform::Rss,
                backend,
                &rss::feedparser_command(query),
                &[],
                deadline,
            )),
            other => Err(unknown_backend(other, Platform::Rss)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "builtin" => {
                let mut h = BackendHealth::new("builtin", Platform::Rss);
                h.available = true;
                h.version = "built-in".to_owned();
                Ok(h)
            }
            "feedparser" => Ok(check_command(
                Platform::Rss,
                backend,
                &["python3", "-c", "import feedparser"],
            )),
            other => Err(unknown_backend(other, Platform::Rss)),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
