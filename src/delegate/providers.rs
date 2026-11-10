//! The four provider kinds: a local CLI, the Anthropic Messages API, Ollama,
//! and any OpenAI-compatible chat endpoint.
//!
//! API keys are read from the environment on every call and sent only in a
//! request header. They never reach a response, an error, or a log.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::DateTime;
use serde_json::{json, Value};

use super::{Class, DelegateError, Provider, ProviderReply, Request};
use crate::config::delegate::DelegateProviderConfig;

/// Output-token allowance when a request sets none.
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 1024;

/// Longest error body echoed into an error message.
const MAX_ERROR_BODY: usize = 300;

/// Environment lookup, injected so tests never read the real environment.
pub type Getenv = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Builds a provider from its config entry.
///
/// # Errors
///
/// A terminal error for a missing field or an unknown type.
pub fn build(
    entry: &DelegateProviderConfig,
    getenv: Getenv,
) -> Result<Box<dyn Provider>, DelegateError> {
    match entry.kind.as_str() {
        "cli" => {
            if entry.command.is_empty() {
                return Err(DelegateError::terminal(format!(
                    "provider {:?}: cli type needs a command",
                    entry.name
                )));
            }
            Ok(Box::new(CliProvider {
                name: entry.name.clone(),
                command: entry.command.clone(),
                args: entry.args.clone(),
            }))
        }
        "anthropic" => {
            require_model(entry)?;
            let key_env = if entry.api_key_env.is_empty() {
                "ANTHROPIC_API_KEY".to_owned()
            } else {
                entry.api_key_env.clone()
            };
            let endpoint = if entry.endpoint.is_empty() {
                "https://api.anthropic.com".to_owned()
            } else {
                entry.endpoint.clone()
            };
            Ok(Box::new(AnthropicProvider {
                name: entry.name.clone(),
                model: entry.model.clone(),
                endpoint,
                key_env,
                getenv,
            }))
        }
        "ollama" => {
            require_model(entry)?;
            let endpoint = if entry.endpoint.is_empty() {
                "http://localhost:11434".to_owned()
            } else {
                entry.endpoint.clone()
            };
            Ok(Box::new(OllamaProvider {
                name: entry.name.clone(),
                model: entry.model.clone(),
                endpoint,
            }))
        }
        "openai" => {
            require_model(entry)?;
            if entry.endpoint.is_empty() {
                return Err(DelegateError::terminal(format!(
                    "provider {:?}: openai type needs an endpoint (the base URL)",
                    entry.name
                )));
            }
            Ok(Box::new(OpenAiProvider {
                name: entry.name.clone(),
                model: entry.model.clone(),
                endpoint: entry.endpoint.clone(),
                key_env: entry.api_key_env.clone(),
                getenv,
            }))
        }
        other => Err(DelegateError::terminal(format!(
            "provider {:?}: unknown type {other:?} (supported: cli, anthropic, ollama, openai)",
            entry.name
        ))),
    }
}

fn require_model(entry: &DelegateProviderConfig) -> Result<(), DelegateError> {
    if entry.model.is_empty() {
        return Err(DelegateError::terminal(format!(
            "provider {:?}: {} type needs a model",
            entry.name, entry.kind
        )));
    }
    Ok(())
}

/// Runs a local CLI with the prompt on stdin. The CLI handles its own auth.
#[derive(Debug)]
pub struct CliProvider {
    name: String,
    command: String,
    args: Vec<String>,
}

impl Provider for CliProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if !req.dir.is_empty() {
            command.current_dir(&req.dir);
        }
        #[cfg(unix)]
        crate::loop_run::jobs::configure_process_group(&mut command);
        #[cfg(windows)]
        crate::loop_run::jobs::configure_suspended_start(&mut command);
        let mut child = command.spawn().map_err(|e| {
            DelegateError::transient(format!(
                "provider {:?}: {} failed: {e}",
                self.name, self.command
            ))
        })?;
        #[cfg(unix)]
        let process_id = child.id();
        #[cfg(windows)]
        let process_job =
            crate::loop_run::jobs::assign_to_kill_on_close_job(&mut child).map_err(|e| {
                let _ = child.kill();
                let _ = child.wait();
                DelegateError::transient(format!(
                    "provider {:?}: process control setup failed: {e}",
                    self.name
                ))
            })?;
        #[cfg(windows)]
        crate::loop_run::jobs::resume_suspended_child(&child).map_err(|e| {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(process_job) };
            let _ = child.kill();
            let _ = child.wait();
            DelegateError::transient(format!(
                "provider {:?}: could not resume managed process: {e}",
                self.name
            ))
        })?;

        let mut stdin = child.stdin.take().expect("stdin was piped");
        let prompt = req.prompt.clone();
        let writer = thread::spawn(move || {
            // A child that exits early closes stdin; the write error is not an answer.
            let _ = stdin.write_all(prompt.as_bytes());
        });
        let mut stdout = child.stdout.take().expect("stdout was piped");
        let mut stderr = child.stderr.take().expect("stderr was piped");
        let out_reader = thread::spawn(move || {
            let mut buf = String::new();
            let _ = stdout.read_to_string(&mut buf);
            buf
        });
        let err_reader = thread::spawn(move || {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf);
            buf
        });

        let started = Instant::now();
        let mut termination_failed = false;
        let mut status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if !req.timeout.is_zero() && started.elapsed() >= req.timeout {
                        #[cfg(unix)]
                        let stopped = crate::loop_run::jobs::terminate_process_tree(process_id);
                        #[cfg(windows)]
                        let stopped = crate::loop_run::jobs::terminate_process_tree(process_job);
                        let _ = child.wait();
                        termination_failed = stopped.is_err();
                        break Err(match stopped {
                            Ok(()) => format!("stopped (timed out after {:?})", req.timeout),
                            Err(error) => format!(
                                "timeout after {:?}; process tree termination failed: {error}",
                                req.timeout
                            ),
                        });
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    #[cfg(unix)]
                    let stopped = crate::loop_run::jobs::terminate_process_tree(process_id);
                    #[cfg(windows)]
                    let stopped = crate::loop_run::jobs::terminate_process_tree(process_job);
                    termination_failed = stopped.is_err();
                    let _ = child.wait();
                    break Err(e.to_string());
                }
            }
        };
        if status.is_ok() {
            while !writer.is_finished() || !out_reader.is_finished() || !err_reader.is_finished() {
                if !req.timeout.is_zero() && started.elapsed() >= req.timeout {
                    #[cfg(unix)]
                    let stopped = crate::loop_run::jobs::terminate_process_tree(process_id);
                    #[cfg(windows)]
                    let stopped = crate::loop_run::jobs::terminate_process_tree(process_job);
                    status = Err(match stopped {
                        Ok(()) => format!(
                            "stopped (timed out while draining output after {:?})",
                            req.timeout
                        ),
                        Err(error) => {
                            termination_failed = true;
                            format!(
                                "timeout after {:?}; process tree termination failed: {error}",
                                req.timeout
                            )
                        }
                    });
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
        if termination_failed {
            #[cfg(windows)]
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(process_job);
            }
            return Err(DelegateError::transient(format!(
                "provider {:?}: {}: {status:?}",
                self.name, self.command
            )));
        }
        let _ = writer.join();
        let stdout_text = out_reader.join().unwrap_or_default();
        let stderr_text = err_reader.join().unwrap_or_default();
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(process_job);
        }

        match status {
            Ok(status) if status.success() => Ok(ProviderReply {
                text: stdout_text.trim().to_owned(),
                input_tokens: None,
                output_tokens: None,
            }),
            // A CLI that is missing, times out, or exits non-zero may not have reached a
            // model, so the next provider in a chain may still answer.
            Ok(status) => Err(DelegateError::transient(format!(
                "provider {:?}: {} failed: exit status {status}: {}",
                self.name,
                self.command,
                stderr_text.trim()
            ))),
            Err(reason) => Err(DelegateError::transient(format!(
                "provider {:?}: {reason}",
                self.name
            ))),
        }
    }
}

/// Calls the Anthropic Messages API.
#[derive(Clone)]
pub struct AnthropicProvider {
    name: String,
    model: String,
    endpoint: String,
    key_env: String,
    getenv: Getenv,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("name", &self.name)
            .field("model", &self.model)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        let key = (self.getenv)(&self.key_env)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                // A missing key is configuration. Another provider will not fix it.
                DelegateError::terminal(format!(
                    "provider {:?}: environment variable {} is not set",
                    self.name, self.key_env
                ))
            })?;
        let body = json!({
            "model": self.model,
            "max_tokens": max_tokens(req),
            "messages": [{ "role": "user", "content": req.prompt }],
        });
        let url = format!("{}/v1/messages", self.endpoint.trim_end_matches('/'));
        let headers = [
            ("content-type", "application/json".to_owned()),
            ("anthropic-version", "2023-06-01".to_owned()),
            ("x-api-key", key),
        ];
        let reply = post_json(&self.name, &url, &headers, &body, req.timeout)?;
        let text: String = reply["content"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter(|p| p["type"] == "text")
                    .filter_map(|p| p["text"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        Ok(ProviderReply {
            text,
            input_tokens: reply["usage"]["input_tokens"].as_u64(),
            output_tokens: reply["usage"]["output_tokens"].as_u64(),
        })
    }
}

/// Calls a local Ollama server. Not billable: the model runs on the user's machine.
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    name: String,
    model: String,
    endpoint: String,
}

impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        let body = json!({
            "model": self.model,
            "prompt": req.prompt,
            "stream": false,
            "options": { "num_predict": max_tokens(req) },
        });
        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let headers = [("content-type", "application/json".to_owned())];
        let reply = post_json(&self.name, &url, &headers, &body, req.timeout)?;
        Ok(ProviderReply {
            text: reply["response"].as_str().unwrap_or("").trim().to_owned(),
            input_tokens: reply["prompt_eval_count"].as_u64(),
            output_tokens: reply["eval_count"].as_u64(),
        })
    }
}

/// Calls an OpenAI-compatible chat completions endpoint (OpenRouter, NVIDIA, Ollama
/// `/v1`, llama.cpp server, LM Studio). The endpoint is the base URL that already
/// holds the version prefix. An empty `key_env` sends no key, which suits a local server.
#[derive(Clone)]
pub struct OpenAiProvider {
    name: String,
    model: String,
    endpoint: String,
    key_env: String,
    getenv: Getenv,
}

impl std::fmt::Debug for OpenAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiProvider")
            .field("name", &self.name)
            .field("model", &self.model)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError> {
        let mut headers = vec![("content-type", "application/json".to_owned())];
        if !self.key_env.is_empty() {
            let key = (self.getenv)(&self.key_env)
                .filter(|k| !k.is_empty())
                .ok_or_else(|| {
                    DelegateError::terminal(format!(
                        "provider {:?}: environment variable {} is not set",
                        self.name, self.key_env
                    ))
                })?;
            headers.push(("authorization", format!("Bearer {key}")));
        }
        let body = json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": req.prompt }],
            "max_tokens": req.max_output_tokens,
        });
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));
        let reply = post_json(&self.name, &url, &headers, &body, req.timeout)?;
        let text = reply["choices"]
            .get(0)
            .and_then(|c| c["message"]["content"].as_str())
            .ok_or_else(|| {
                DelegateError::terminal(format!(
                    "provider {:?}: response has no choices",
                    self.name
                ))
            })?;
        Ok(ProviderReply {
            text: text.trim().to_owned(),
            input_tokens: reply["usage"]["prompt_tokens"].as_u64(),
            output_tokens: reply["usage"]["completion_tokens"].as_u64(),
        })
    }
}

fn max_tokens(req: &Request) -> u32 {
    if req.max_output_tokens == 0 {
        DEFAULT_MAX_OUTPUT_TOKENS
    } else {
        req.max_output_tokens
    }
}

/// POSTs JSON and returns the decoded 2xx body. A non-2xx response is classified
/// by [`http_error`]. A transport failure is transient. A body that does not decode
/// is terminal, because retrying it elsewhere would repeat the call.
fn post_json(
    provider: &str,
    url: &str,
    headers: &[(&str, String)],
    body: &Value,
    timeout: Duration,
) -> Result<Value, DelegateError> {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global((!timeout.is_zero()).then_some(timeout))
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent.post(url);
    for (name, value) in headers {
        request = request.header(*name, value.as_str());
    }
    let mut response = request.send(body.to_string()).map_err(|e| {
        DelegateError::transient(format!("provider {provider:?}: request failed: {e}"))
    })?;

    let status = response.status().as_u16();
    let retry = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let text = response.body_mut().read_to_string().map_err(|e| {
        DelegateError::transient(format!("provider {provider:?}: reading response: {e}"))
    })?;
    if !(200..=299).contains(&status) {
        return Err(http_error(provider, status, retry.as_deref(), &text));
    }
    serde_json::from_str(&text).map_err(|e| {
        DelegateError::terminal(format!("provider {provider:?}: decoding response: {e}"))
    })
}

/// Classifies a non-2xx response. 429 is rate limiting. 5xx is transient. Any other
/// 4xx is terminal, because the same request would fail elsewhere too.
#[must_use]
pub fn http_error(
    provider: &str,
    status: u16,
    retry_after: Option<&str>,
    body: &str,
) -> DelegateError {
    let message = format!("provider {provider:?}: HTTP {status}: {}", truncate(body));
    match status {
        429 => DelegateError {
            class: Class::RateLimited,
            retry_after: retry_after.and_then(parse_retry_after),
            message,
        },
        500..=599 => DelegateError::transient(message),
        _ => DelegateError::terminal(message),
    }
}

/// Reads `Retry-After` as seconds, or as an HTTP date. `None` when absent or unusable.
#[must_use]
pub fn parse_retry_after(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Ok(secs) = value.parse::<u64>() {
        return (secs > 0).then(|| Duration::from_secs(secs));
    }
    let when = DateTime::parse_from_rfc2822(value).ok()?;
    let delta = when.timestamp() - chrono::Utc::now().timestamp();
    (delta > 0).then(|| Duration::from_secs(delta as u64))
}

fn truncate(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.chars().count() > MAX_ERROR_BODY {
        let cut: String = trimmed.chars().take(MAX_ERROR_BODY).collect();
        format!("{cut}...")
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_classes_match_the_chain_rules() {
        assert_eq!(
            http_error("p", 429, Some("7"), "").class,
            Class::RateLimited
        );
        assert_eq!(
            http_error("p", 429, Some("7"), "").retry_after,
            Some(Duration::from_secs(7))
        );
        assert_eq!(http_error("p", 503, None, "").class, Class::Transient);
        assert_eq!(http_error("p", 401, None, "").class, Class::Terminal);
    }

    #[test]
    fn retry_after_accepts_seconds_and_rejects_nonsense() {
        assert_eq!(parse_retry_after("12"), Some(Duration::from_secs(12)));
        assert_eq!(parse_retry_after("0"), None);
        assert_eq!(parse_retry_after("soon"), None);
    }

    #[test]
    fn error_bodies_are_capped() {
        let long = "x".repeat(500);
        assert!(http_error("p", 400, None, &long).message.len() < 400);
    }

    #[test]
    fn missing_model_is_a_configuration_error() {
        let entry = DelegateProviderConfig {
            name: "a".into(),
            kind: "anthropic".into(),
            ..Default::default()
        };
        let err = build(&entry, std::sync::Arc::new(|_: &str| None))
            .err()
            .expect("refused");
        assert_eq!(err.class, Class::Terminal);
    }

    #[test]
    fn cli_provider_runs_a_local_command_with_the_prompt_on_stdin() {
        let entry = DelegateProviderConfig {
            name: "echo".into(),
            kind: "cli".into(),
            command: if cfg!(windows) {
                "cmd".into()
            } else {
                "cat".into()
            },
            args: if cfg!(windows) {
                vec!["/C".into(), "more".into()]
            } else {
                vec![]
            },
            ..Default::default()
        };
        let provider = build(&entry, std::sync::Arc::new(|_: &str| None)).unwrap();
        let reply = provider
            .complete(&Request {
                prompt: "hello there".into(),
                ..Request::default()
            })
            .unwrap();
        assert_eq!(reply.text, "hello there");
    }
}
