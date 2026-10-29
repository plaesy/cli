//! Adapters for Chinese platforms: Bilibili, V2EX, Xueqiu, Xiaoyuzhou, and BOSS Zhipin.

use std::collections::BTreeMap;

use crate::reach::adapter::{opt, opt_or, push_flag, unknown_backend, PlatformAdapter};
use crate::reach::exec::{argv, check_command, run_backend, Deadline};
use crate::reach::types::{BackendHealth, BackendResult, Options, Platform};
use crate::reach::ReachError;

fn unknown_action(action: &str, context: &str) -> ReachError {
    ReachError::UnknownAction {
        action: action.to_owned(),
        context: context.to_owned(),
    }
}

/// Bilibili through bili-cli, opencli, or the public web API.
pub struct BilibiliAdapter;

impl PlatformAdapter for BilibiliAdapter {
    fn name(&self) -> Platform {
        Platform::Bilibili
    }

    fn backends(&self) -> &'static [&'static str] {
        &["bili-cli", "opencli", "bili-api"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "bili-cli" => {
                let action = opt_or(opts, "action", "search");
                let mut args = argv(&["bili", action]);
                if !query.is_empty() {
                    args.push(query.to_owned());
                }
                push_flag(&mut args, "--limit", opt(opts, "limit"));
                Ok(run_backend(
                    Platform::Bilibili,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "opencli" => {
                let mut args = argv(&["opencli", "bilibili"]);
                let action = opt(opts, "action");
                if !action.is_empty() {
                    args.push(action.to_owned());
                }
                if !query.is_empty() {
                    args.push(query.to_owned());
                }
                Ok(run_backend(
                    Platform::Bilibili,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "bili-api" => {
                let action = opt_or(opts, "action", "search");
                let url = match action {
                    "search" => format!("https://api.bilibili.com/x/web-interface/search/type?keyword={query}&page=1"),
                    "video" => format!("https://api.bilibili.com/x/web-interface/view?bvid={query}"),
                    other => return Err(unknown_action(other, "bili-api")),
                };
                Ok(run_backend(
                    Platform::Bilibili,
                    backend,
                    &argv(&["curl", "-s", &url]),
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::Bilibili)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "bili-cli" => Ok(check_command(
                Platform::Bilibili,
                backend,
                &["bili", "--version"],
            )),
            "opencli" => Ok(check_command(
                Platform::Bilibili,
                backend,
                &["opencli", "--version"],
            )),
            "bili-api" => Ok(check_command(
                Platform::Bilibili,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Bilibili)),
        }
    }
}

/// V2EX topics, nodes, and members through its public JSON API.
pub struct V2exAdapter;

impl PlatformAdapter for V2exAdapter {
    fn name(&self) -> Platform {
        Platform::V2ex
    }

    fn backends(&self) -> &'static [&'static str] {
        &["v2ex-api"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "v2ex-api" => {
                let action = opt_or(opts, "action", "topics");
                let url = match action {
                    "topics" => "https://www.v2ex.com/api/topics/latest.json".to_owned(),
                    "nodes" => "https://www.v2ex.com/api/nodes/all.json".to_owned(),
                    "node" => format!("https://www.v2ex.com/api/nodes/show.json?name={query}"),
                    "topic" => format!("https://www.v2ex.com/api/topics/show.json?id={query}"),
                    "user" => {
                        format!("https://www.v2ex.com/api/members/show.json?username={query}")
                    }
                    other => return Err(unknown_action(other, "v2ex")),
                };
                Ok(run_backend(
                    Platform::V2ex,
                    backend,
                    &argv(&["curl", "-s", &url]),
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::V2ex)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "v2ex-api" => Ok(check_command(
                Platform::V2ex,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::V2ex)),
        }
    }
}

/// Xueqiu (stock forum) through its web API.
pub struct XueqiuAdapter;

impl PlatformAdapter for XueqiuAdapter {
    fn name(&self) -> Platform {
        Platform::Xueqiu
    }

    fn backends(&self) -> &'static [&'static str] {
        &["xueqiu-api"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "xueqiu-api" => {
                let action = opt_or(opts, "action", "search");
                let url = match action {
                    "search" => format!("https://xueqiu.com/stock/search.json?q={query}"),
                    "stock" => {
                        format!("https://stock.xueqiu.com/v5/stock/quote.json?symbol={query}")
                    }
                    "timeline" => {
                        format!("https://xueqiu.com/v4/statuses/user_timeline.json?user_id={query}")
                    }
                    "comments" => {
                        format!("https://xueqiu.com/query/v1/symbol/search/status.json?q={query}")
                    }
                    other => return Err(unknown_action(other, "xueqiu")),
                };
                let args = argv(&["curl", "-s", "-H", "User-Agent: Mozilla/5.0", &url]);
                Ok(run_backend(Platform::Xueqiu, backend, &args, &[], deadline))
            }
            other => Err(unknown_backend(other, Platform::Xueqiu)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "xueqiu-api" => Ok(check_command(
                Platform::Xueqiu,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Xueqiu)),
        }
    }
}

/// Xiaoyuzhou (podcasts) through the mcporter bridge or its web API.
#[derive(Default)]
pub struct XiaoyuzhouAdapter {
    whisper_key: String,
}

impl PlatformAdapter for XiaoyuzhouAdapter {
    fn name(&self) -> Platform {
        Platform::Xiaoyuzhou
    }

    fn backends(&self) -> &'static [&'static str] {
        &["xiaoyuzhou-mcp", "xiaoyuzhou-api"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "xiaoyuzhou-mcp" => {
                let action = opt_or(opts, "action", "search");
                let mut args = argv(&["mcporter", "call", "xiaoyuzhou", action, query]);
                push_flag(&mut args, "--whisper-key", &self.whisper_key);
                Ok(run_backend(
                    Platform::Xiaoyuzhou,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "xiaoyuzhou-api" => {
                let action = opt_or(opts, "action", "search");
                let url = match action {
                    "search" => format!("https://www.xiaoyuzhoufm.com/api/search?q={query}"),
                    "podcast" => format!("https://www.xiaoyuzhoufm.com/api/podcast/{query}"),
                    "episode" => format!("https://www.xiaoyuzhoufm.com/api/episode/{query}"),
                    other => return Err(unknown_action(other, "xiaoyuzhou")),
                };
                Ok(run_backend(
                    Platform::Xiaoyuzhou,
                    backend,
                    &argv(&["curl", "-s", &url]),
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::Xiaoyuzhou)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "xiaoyuzhou-mcp" => Ok(check_command(
                Platform::Xiaoyuzhou,
                backend,
                &["mcporter", "--version"],
            )),
            "xiaoyuzhou-api" => Ok(check_command(
                Platform::Xiaoyuzhou,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::Xiaoyuzhou)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        if let Some(v) = creds.get("whisper_key") {
            self.whisper_key = v.clone();
        }
        Ok(())
    }
}

/// BOSS Zhipin (jobs) through a browser automation CLI, or its web API.
#[derive(Default)]
pub struct BossZhipinAdapter {
    cookie: String,
}

impl PlatformAdapter for BossZhipinAdapter {
    fn name(&self) -> Platform {
        Platform::BossZhipin
    }

    fn backends(&self) -> &'static [&'static str] {
        &["boss-cdp", "boss-api"]
    }

    fn execute(
        &self,
        backend: &str,
        query: &str,
        opts: &Options,
        deadline: Deadline,
    ) -> Result<BackendResult, ReachError> {
        match backend {
            "boss-cdp" => {
                let action = opt_or(opts, "action", "search");
                let mut args = argv(&["boss-cdp", action, query]);
                push_flag(&mut args, "--cookie", &self.cookie);
                Ok(run_backend(
                    Platform::BossZhipin,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            "boss-api" => {
                let action = opt_or(opts, "action", "search");
                let url = match action {
                    "search" => format!(
                        "https://www.zhipin.com/wapi/zpgeek/search/joblist.json?query={query}"
                    ),
                    "job" => format!(
                        "https://www.zhipin.com/wapi/zpgeek/view/job/card.json?jobId={query}"
                    ),
                    other => return Err(unknown_action(other, "bosszhipin")),
                };
                let mut args = argv(&["curl", "-s", "-H", "User-Agent: Mozilla/5.0"]);
                if !self.cookie.is_empty() {
                    args.push("-H".to_owned());
                    args.push(format!("Cookie: {}", self.cookie));
                }
                args.push(url);
                Ok(run_backend(
                    Platform::BossZhipin,
                    backend,
                    &args,
                    &[],
                    deadline,
                ))
            }
            other => Err(unknown_backend(other, Platform::BossZhipin)),
        }
    }

    fn health(&self, backend: &str) -> Result<BackendHealth, ReachError> {
        match backend {
            "boss-cdp" => Ok(check_command(
                Platform::BossZhipin,
                backend,
                &["boss-cdp", "--version"],
            )),
            "boss-api" => Ok(check_command(
                Platform::BossZhipin,
                backend,
                &["curl", "--version"],
            )),
            other => Err(unknown_backend(other, Platform::BossZhipin)),
        }
    }

    fn configure(&mut self, creds: &BTreeMap<String, String>) -> Result<(), ReachError> {
        if let Some(v) = creds.get("cookie") {
            self.cookie = v.clone();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_action_names_the_backend() {
        let deadline = Deadline::after(std::time::Duration::from_secs(1));
        let mut opts = Options::new();
        opts.insert("action".into(), "delete".into());
        let err = V2exAdapter
            .execute("v2ex-api", "x", &opts, deadline)
            .expect_err("unknown action");
        assert!(err.to_string().contains("v2ex"));
    }
}
