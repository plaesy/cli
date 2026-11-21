//! `plaesy advisor`: read-only model advice for general choices and task triage.

use std::path::{Path, PathBuf};

use super::CliResult;
use crate::tasks;

/// Resolve an existing task by its folder name or unique title.
fn resolve_task_directory(tasks_dir: &std::path::Path, query: &str) -> Result<String, String> {
    if tasks::find_task(tasks_dir, query).is_some() {
        return Ok(query.to_owned());
    }
    let query = query.trim();
    if query.is_empty() {
        return Err("task name or title must not be empty".into());
    }
    let mut matches = Vec::new();
    for status in tasks::STATUSES {
        for name in tasks::list_tasks(tasks_dir, status)
            .map_err(|e| e.to_string())?
            .unwrap_or_default()
        {
            let Some((found_status, _)) = tasks::find_task(tasks_dir, &name) else {
                continue;
            };
            if found_status != status {
                continue;
            }
            let title = tasks::read_task_frontmatter(tasks_dir, found_status, &name)
                .ok()
                .and_then(|frontmatter| frontmatter.get("title").cloned())
                .unwrap_or_default();
            if title.eq_ignore_ascii_case(query) {
                matches.push(name);
            }
        }
    }
    match matches.as_slice() {
        [name] => Ok(name.clone()),
        [] => Err(format!("task not found by name or title: {query}")),
        _ => Err(format!(
            "task title is ambiguous: {query}; matching task IDs: {}",
            matches.join(", ")
        )),
    }
}

fn read_task_body(tasks_dir: &Path, status: tasks::Status, task: &str) -> Result<String, String> {
    let root = crate::memory::fs_safety::DirHandle::open_nofollow(tasks_dir)
        .map_err(|error| format!("opening tasks directory safely: {error}"))?;
    let relative = Path::new(status.as_str()).join(task).join(tasks::TASK_FILE);
    let mut file = root
        .open_file_read_relative_nofollow(&relative)
        .map_err(|error| format!("reading task {task}: {error}"))?;
    let mut body = String::new();
    std::io::Read::read_to_string(&mut file, &mut body)
        .map_err(|error| format!("reading task {task}: {error}"))?;
    Ok(body)
}

/// Ask a configured delegate for advice without mutating project state.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: AdvisorCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum AdvisorCommand {
    /// Compare explicitly supplied general options.
    Compare(CompareArgs),
    /// Recommend classification and priority for one existing task; never edits it.
    Triage(TriageArgs),
}

#[derive(Debug, clap::Args)]
pub struct CompareArgs {
    /// A question or decision to make.
    #[arg(long)]
    pub prompt: String,
    /// Candidate option (repeat for each option; provide at least two).
    #[arg(long = "option", required = true, num_args = 1, action = clap::ArgAction::Append)]
    pub options: Vec<String>,
    /// Laya SystemOne endpoint (e.g. http://127.0.0.1:8000/v1/systemone).
    /// When set, use typed Laya choice output instead of a text delegate.
    #[arg(long, conflicts_with_all = ["provider", "chain"])]
    pub laya_endpoint: Option<String>,
    #[command(flatten)]
    pub delegate: DelegateArgs,
}

#[derive(Debug, clap::Args)]
pub struct TriageArgs {
    /// Existing task folder name or unique task title.
    pub task: String,
    #[command(flatten)]
    pub delegate: DelegateArgs,
}

#[derive(Debug, clap::Args)]
pub struct DelegateArgs {
    /// Project directory (defaults to current project).
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Configured delegate provider.
    #[arg(long, conflicts_with = "chain")]
    pub provider: Option<String>,
    /// Configured delegate chain.
    #[arg(long, conflicts_with = "provider")]
    pub chain: Option<String>,
    /// Allow billable providers to run.
    #[arg(long)]
    pub confirm: bool,
    /// Call timeout.
    #[arg(long, default_value = "120s")]
    pub timeout: String,
    /// Maximum output tokens (zero uses provider default).
    #[arg(long, default_value_t = 0)]
    pub max_tokens: u32,
}

fn advice(delegate: &DelegateArgs, prompt: String) -> CliResult {
    let provider = delegate.provider.clone().unwrap_or_default();
    let chain = delegate.chain.clone().unwrap_or_default();
    if provider.is_empty() && chain.is_empty() {
        return Err("select a configured delegate with --provider NAME or --chain NAME".into());
    }
    super::delegate::run(&super::delegate::Args {
        command: None,
        path: delegate.path.clone(),
        provider,
        chain,
        prompt,
        max_tokens: delegate.max_tokens,
        timeout: delegate.timeout.clone(),
        confirm: delegate.confirm,
        out: None,
        list: false,
    })
}

fn compare_laya(args: &CompareArgs, endpoint: &str) -> CliResult {
    if !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
        return Err("--laya-endpoint must be an http:// or https:// URL".into());
    }
    let criteria = args
        .options
        .iter()
        .map(|option| (option.clone(), option.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let body = serde_json::json!({
        "state": args.prompt,
        "questions": {
            "recommendation": {
                "type": "choice",
                "instructions": "Choose the candidate option that best answers the decision. Do not invent options.",
                "criteria": criteria
            }
        }
    });
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .post(endpoint)
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| format!("calling Laya endpoint {endpoint}: {e}"))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string()?;
    if !(200..=299).contains(&status) {
        return Err(format!("Laya endpoint returned HTTP {status}: {text}").into());
    }
    let result: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("decoding Laya response: {e}"))?;
    let answer = result
        .pointer("/answers/recommendation")
        .ok_or("Laya response is missing answers.recommendation")?;
    let choice = answer
        .get("choice")
        .and_then(serde_json::Value::as_str)
        .ok_or("Laya response is missing a string choice")?;
    if !args.options.iter().any(|option| option == choice) {
        return Err(format!("Laya returned an option that was not supplied: {choice}").into());
    }
    println!("Laya choice: {choice}");
    println!("{}", serde_json::to_string_pretty(answer)?);
    Ok(())
}

fn compare(args: &CompareArgs) -> CliResult {
    if args.options.len() < 2 {
        return Err("provide at least two --option values".into());
    }
    if let Some(endpoint) = args.laya_endpoint.as_deref() {
        return compare_laya(args, endpoint);
    }
    let options = args
        .options
        .iter()
        .enumerate()
        .map(|(i, option)| format!("{}. {}", i + 1, option))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "You are Plaesy's read-only Advisor. Compare only the candidate options below for this decision: {}\n\nOptions:\n{}\n\nReturn a concise, structured comparison with criteria, trade-offs, assumptions, missing information, and a recommendation only if justified. Distinguish facts from assumptions; do not claim external research you did not perform. Do not execute actions or change configuration.",
        args.prompt, options
    );
    advice(&args.delegate, prompt)
}

fn triage(args: &TriageArgs) -> CliResult {
    let path = args.delegate.path.canonicalize()?;
    let root = tasks::find_project_root(&path)?;
    let tasks_dir = tasks::tasks_dir(&root);
    let task_name = resolve_task_directory(&tasks_dir, &args.task)?;
    let Some((status, _task_dir)) = tasks::find_task(&tasks_dir, &task_name) else {
        return Err(format!("task not found: {task_name}").into());
    };
    let body = read_task_body(&tasks_dir, status, &task_name).map_err(std::io::Error::other)?;
    let issue = tasks::issue::Issue::read(&tasks_dir, status, &task_name)?;
    let metadata = serde_json::to_value(issue)?;
    let prompt = format!(
        "You are Plaesy's read-only task triage Advisor. Review the task below and recommend an issue type (issue, story, bug, epic, task, or subtask), priority (critical, high, medium, or low), concise rationale, uncertainties, and any clarifying question. Do not edit the task or claim to have done so.\n\nTask ID: {}\nCurrent status: {}\nMetadata: {}\n\nTask content:\n{}",
        task_name,
        status.as_str(),
        serde_json::to_string_pretty(&metadata)?,
        body
    );
    advice(&args.delegate, prompt)
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        AdvisorCommand::Compare(args) => compare(args),
        AdvisorCommand::Triage(args) => triage(args),
    }
}

#[cfg(test)]
#[path = "advisor_test.rs"]
mod advisor_test;
