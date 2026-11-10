//! Adapters for social and search platforms that need credentials: Twitter, Reddit, Facebook,
//! Instagram, LinkedIn, XiaoHongShu, and Exa.

use std::collections::BTreeMap;

use crate::reach::adapter::{opt, opt_or, push_flag, unknown_backend, PlatformAdapter};
use crate::reach::exec::{argv, check_command, run_backend, Deadline};
use crate::reach::types::{BackendHealth, BackendResult, Options, Platform};
use crate::reach::ReachError;

/// The `opencli <platform> [action] [query]` command most social adapters use.
fn opencli(platform: &str, action: &str, query: &str, cookie: &str) -> Vec<String> {
    let mut args = argv(&["opencli", platform]);
    if !action.is_empty() {
        args.push(action.to_owned());
    }
    if !query.is_empty() {
        args.push(query.to_owned());
    }
    push_flag(&mut args, "--cookie", cookie);
    args
}

fn cookie_creds(creds: &BTreeMap<String, String>, cookie: &mut String) {
    if let Some(v) = creds.get("cookie") {
        *cookie = v.clone();
    }
}

/// Twitter/X through twitter-cli, opencli, or bird.
#[derive(Default)]
pub struct TwitterAdapter {
    auth_token: String,
    ct0: String,
}

impl PlatformAdapter for TwitterAdapter {
    fn name(&self) -> Platform {
        Platform::Twitter
    }

    fn backends(&self) -> &'static [&'static str] {
        &["twitter-cli", "opencli", "bird"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "twitter-cli" => {
                let action = opt_or(opts, "action", "search");
                let mut args = argv(&["twitter"]);
                match action {
                    "tweet" => args.extend(argv(&["tweet", "view", query])),
                    "user" => args.extend(argv(&["user", "view", query])),
                    "search" => {
                        args.extend(argv(&["search", query]));
                        push_flag(&mut args, "--limit", opt(opts, "limit"));
                    }
                    "timeline" => args.push("timeline".to_owned()),
                    other => {
                        return Err(ReachError::UnknownAction {
                            action: other.to_owned(),
                            context: "twitter-cli".to_owned(),
                        })
                    }
                }
                let mut env = Vec::new();
                if !self.auth_token.is_empty() {
                    env.push(("TWITTER_AUTH_TOKEN".to_owned(), self.auth_token.clone()));
                }
                if !self.ct0.is_empty() {
                    env.push(("TWITTER_CT0".to_owned(), self.ct0.clone()));
                }
                Ok(run_backend(
                    Platform::Twitter,
                    backend,
                    &args,
                    &env,
                    deadline,
                ))
            }
            "opencli" => {
                let args = opencli("twitter", opt(opts, "action"), query, "");
                Ok(run_backend(
                    Platform::Twitter,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "bird" => {
                let action = opt_or(opts, "action", "search");
                Ok(run_backend(
                    Platform::Twitter,
                    backend,
                    &argv(&["bird", action, query]),
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::Twitter)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "twitter-cli" => Ok(check_command(
                Platform::Twitter,
                backend,
                &["twitter", "--version"],
            )),
            "opencli" => Ok(check_command(
                Platform::Twitter,
                backend,
                &["opencli", "--version"],
            )),
            "bird" => Ok(check_command(
                Platform::Twitter,
                backend,
                &["bird", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Twitter)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        if let Some(v) = creds.get("auth_token") {
            self.auth_token = v.clone();
        }
        if let Some(v) = creds.get("ct0") {
            self.ct0 = v.clone();
        }
        Ok(())
    }
}

/// Reddit through opencli or rdt-cli.
#[derive(Default)]
pub struct RedditAdapter {
    cookie: String,
}

impl PlatformAdapter for RedditAdapter {
    fn name(&self) -> Platform {
        Platform::Reddit
    }

    fn backends(&self) -> &'static [&'static str] {
        &["opencli", "rdt-cli"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "opencli" => Ok(run_backend(
                Platform::Reddit,
                backend,
                &opencli("reddit", opt(opts, "action"), query, ""),
                &[],
                deadline,
            )),
            "rdt-cli" => {
                let action = opt_or(opts, "action", "search");
                let mut args = argv(&["rdt", action]);
                if !query.is_empty() {
                    args.push(query.to_owned());
                }
                push_flag(&mut args, "--cookie", &self.cookie);
                Ok(run_backend(Platform::Reddit, backend, &args, &[], deadline))
            }
            other => Err(unknown_backend(other, Platform::Reddit)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "opencli" => Ok(check_command(
                Platform::Reddit,
                backend,
                &["opencli", "--version"],
            )),
            "rdt-cli" => Ok(check_command(
                Platform::Reddit,
                backend,
                &["rdt", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Reddit)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        cookie_creds(creds, &mut self.cookie);
        Ok(())
    }
}

/// Facebook through opencli.
#[derive(Default)]
pub struct FacebookAdapter {
    cookie: String,
}

impl PlatformAdapter for FacebookAdapter {
    fn name(&self) -> Platform {
        Platform::Facebook
    }

    fn backends(&self) -> &'static [&'static str] {
        &["opencli"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "opencli" => {
                let args = opencli("facebook", opt(opts, "action"), query, &self.cookie);
                Ok(run_backend(
                    Platform::Facebook,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::Facebook)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "opencli" => Ok(check_command(
                Platform::Facebook,
                backend,
                &["opencli", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Facebook)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        cookie_creds(creds, &mut self.cookie);
        Ok(())
    }
}

/// Instagram through opencli.
#[derive(Default)]
pub struct InstagramAdapter {
    cookie: String,
}

impl PlatformAdapter for InstagramAdapter {
    fn name(&self) -> Platform {
        Platform::Instagram
    }

    fn backends(&self) -> &'static [&'static str] {
        &["opencli"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "opencli" => {
                let args = opencli("instagram", opt(opts, "action"), query, &self.cookie);
                Ok(run_backend(
                    Platform::Instagram,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::Instagram)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "opencli" => Ok(check_command(
                Platform::Instagram,
                backend,
                &["opencli", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Instagram)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        cookie_creds(creds, &mut self.cookie);
        Ok(())
    }
}

/// LinkedIn through the mcporter MCP bridge, or Jina Reader.
#[derive(Default)]
pub struct LinkedInAdapter {
    mcp_config: String,
}

impl PlatformAdapter for LinkedInAdapter {
    fn name(&self) -> Platform {
        Platform::LinkedIn
    }

    fn backends(&self) -> &'static [&'static str] {
        &["mcp-linkedin", "jina-reader"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "mcp-linkedin" => {
                let action = opt_or(opts, "action", "search");
                Ok(run_backend(
                    Platform::LinkedIn,
                    backend,
                    &argv(&["mcporter", "call", "linkedin", action, query]),
                    &[],
                    deadline,
                ))
            }
            "jina-reader" => {
                let url = if query.starts_with("http") {
                    query.to_owned()
                } else {
                    format!("https://www.linkedin.com/{query}")
                };
                let jina = format!("https://r.jina.ai/{url}");
                Ok(run_backend(
                    Platform::LinkedIn,
                    backend,
                    &argv(&["curl", "-s", "-L", &jina]),
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::LinkedIn)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "mcp-linkedin" => Ok(check_command(
                Platform::LinkedIn,
                backend,
                &["mcporter", "--version"],
            )),
            "jina-reader" => Ok(check_command(
                Platform::LinkedIn,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::LinkedIn)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        if let Some(v) = creds.get("mcp_config") {
            self.mcp_config = v.clone();
        }
        Ok(())
    }
}

/// XiaoHongShu through opencli, its MCP server, or xhs-cli.
#[derive(Default)]
pub struct XiaoHongShuAdapter {
    cookie: String,
}

impl PlatformAdapter for XiaoHongShuAdapter {
    fn name(&self) -> Platform {
        Platform::XiaoHongShu
    }

    fn backends(&self) -> &'static [&'static str] {
        &["opencli", "xiaohongshu-mcp", "xhs-cli"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "opencli" => {
                let args = opencli("xiaohongshu", opt(opts, "action"), query, "");
                Ok(run_backend(
                    Platform::XiaoHongShu,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "xiaohongshu-mcp" => {
                let mut args = argv(&["mcporter", "call", "xiaohongshu", "search", query]);
                push_flag(&mut args, "--cookie", &self.cookie);
                Ok(run_backend(
                    Platform::XiaoHongShu,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "xhs-cli" => {
                let action = opt_or(opts, "action", "search");
                let mut args = argv(&["xhs", action, query]);
                push_flag(&mut args, "--cookie", &self.cookie);
                Ok(run_backend(
                    Platform::XiaoHongShu,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::XiaoHongShu)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "opencli" => Ok(check_command(
                Platform::XiaoHongShu,
                backend,
                &["opencli", "--version"],
            )),
            "xiaohongshu-mcp" => Ok(check_command(
                Platform::XiaoHongShu,
                backend,
                &["mcporter", "--version"],
            )),
            "xhs-cli" => Ok(check_command(
                Platform::XiaoHongShu,
                backend,
                &["xhs", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::XiaoHongShu)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        cookie_creds(creds, &mut self.cookie);
        Ok(())
    }
}

/// Exa semantic search, through the mcporter bridge or the REST API.
#[derive(Default)]
pub struct ExaSearchAdapter {
    api_key: String,
}

impl PlatformAdapter for ExaSearchAdapter {
    fn name(&self) -> Platform {
        Platform::ExaSearch
    }

    fn backends(&self) -> &'static [&'static str] {
        &["exa-mcp", "exa-api"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "exa-mcp" => {
                let mut args = argv(&["mcporter", "call", "exa", "search", query]);
                push_flag(&mut args, "--num-results", opt(opts, "num_results"));
                push_flag(&mut args, "--include-domains", opt(opts, "include_domains"));
                push_flag(&mut args, "--exclude-domains", opt(opts, "exclude_domains"));
                push_flag(&mut args, "--category", opt(opts, "category"));
                Ok(run_backend(
                    Platform::ExaSearch,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "exa-api" => {
                if self.api_key.is_empty() {
                    let mut result = BackendResult::new("exa-api", Platform::ExaSearch);
                    result.error = "Exa API key not configured".to_owned();
                    return Ok(result);
                }
                let body = format!(r#"{{"query": "{query}", "numResults": 10}}"#);
                let args = argv(&[
                    "curl",
                    "-s",
                    "-X",
                    "POST",
                    "https://api.exa.ai/search",
                    "-H",
                    "Content-Type: application/json",
                    "-H",
                    &format!("Authorization: Bearer {}", self.api_key),
                    "-d",
                    &body,
                ]);
                Ok(run_backend(
                    Platform::ExaSearch,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::ExaSearch)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "exa-mcp" => Ok(check_command(
                Platform::ExaSearch,
                backend,
                &["mcporter", "--version"],
            )),
            "exa-api" if self.api_key.is_empty() => {
                let mut h = BackendHealth::new("exa-api", Platform::ExaSearch);
                h.error = "API key not configured".to_owned();
                Ok(h)
            }
            "exa-api" => Ok(check_command(
                Platform::ExaSearch,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::ExaSearch)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        if let Some(v) = creds.get("api_key") {
            self.api_key = v.clone();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exa_without_a_key_reports_it_instead_of_calling_out() {
        let adapter = ExaSearchAdapter::default();
        let deadline = Deadline::after(std::time::Duration::from_secs(5));
        let result = adapter
            .execute("exa-api", "rust", &Options::new(), deadline)
            .expect("known backend");
        assert!(!result.success);
        assert!(result.error.contains("API key"));
    }

    #[test]
    fn opencli_builds_the_platform_command() {
        let args = opencli("reddit", "search", "rust", "c=1");
        assert_eq!(
            args,
            argv(&["opencli", "reddit", "search", "rust", "--cookie", "c=1"])
        );
    }
}
