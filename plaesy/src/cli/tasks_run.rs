//! `plaesy tasks run`: claims todo tasks of a spec and hands each to a delegate provider or chain.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;

use super::common::find_root;
use super::delegate::{chain_names, load_delegate_config};
use super::CliResult;
use crate::config::delegate::DelegateConfig;
use crate::delegate::{self, usage, Attempt, ChainOptions, Request, Response};
use crate::tasks;

/// Claims todo tasks of the spec (highest priority first) and sends each to a delegate provider
/// or chain. Each claim is an atomic move todo -> doing. A failed run moves the task to blocked.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Provider name from delegate.providers
    #[arg(long, default_value = "")]
    pub provider: String,
    /// Chain name from delegate.chains
    #[arg(long, default_value = "")]
    pub chain: String,
    /// Stop after this many tasks
    #[arg(long = "max", default_value_t = 1)]
    pub max_tasks: usize,
    /// Run this many tasks at once
    #[arg(long, default_value_t = 1)]
    pub parallel: usize,
    /// Stop each call after this long (e.g. 10m)
    #[arg(long, default_value = "10m")]
    pub timeout: String,
    /// Allow a billable provider to run
    #[arg(long)]
    pub confirm: bool,
}

/// Runs the batch. `spec` is the spec the claims are made in.
pub fn run(args: &Args, spec: &str) -> CliResult {
    if args.provider.is_empty() && args.chain.is_empty() {
        return Err(
            "exactly one of --provider or --chain is required (see `plaesy delegate --list`)"
                .into(),
        );
    }
    let root = find_root()?;
    let tasks_dir = tasks::tasks_dir(&root);
    let cfg = load_delegate_config(&root)?;
    let names = chain_names(&cfg, &args.provider, &args.chain)?;
    let timeout = crate::reach::duration::parse(&args.timeout)
        .ok_or_else(|| format!("invalid --timeout {:?}", args.timeout))?;
    let workers = args.parallel.max(1);

    let started = AtomicUsize::new(0);
    let ran = AtomicUsize::new(0);
    let first_error: Mutex<Option<String>> = Mutex::new(None);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                if started.fetch_add(1, Ordering::SeqCst) >= args.max_tasks {
                    return;
                }
                let claimed = tasks::claim::claim_task(&tasks_dir, spec);
                let outcome = match claimed {
                    Ok(None) => return,
                    Ok(Some(file)) => {
                        let res =
                            run_claimed_task(&root, &tasks_dir, &file, &cfg, &names, args, timeout);
                        ran.fetch_add(1, Ordering::SeqCst);
                        res
                    }
                    Err(e) => Err(e.to_string()),
                };
                if let Err(message) = outcome {
                    let mut slot = first_error
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if slot.is_none() {
                        *slot = Some(message);
                    }
                    return;
                }
            });
        }
    });

    if let Some(message) = first_error
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        return Err(message.into());
    }
    if ran.load(Ordering::SeqCst) == 0 {
        println!("no todo or backlog tasks in this spec");
    }
    Ok(())
}

/// Sends one claimed task to the chain, saves the run, and moves the task on. An `Err` is an I/O
/// failure; a failed run is recorded on the task and does not stop the batch.
fn run_claimed_task(
    root: &Path,
    tasks_dir: &Path,
    file: &str,
    cfg: &DelegateConfig,
    names: &[String],
    args: &Args,
    timeout: Duration,
) -> Result<(), String> {
    let task_dir = tasks_dir.join("doing").join(file);
    // The prompt is the task's topic spec.
    let topic = crate::tasks::topic_of(tasks_dir, crate::tasks::Status::Doing, file)
        .ok_or_else(|| format!("{file} has no topic; every task needs one"))?;
    let body = fs::read_to_string(crate::tasks::spec_doc(root, &topic)).map_err(|e| e.to_string())?;
    let prompt = build_task_prompt(file, &body);

    let usage_path = usage::usage_path(&root.join(".plaesy").join("analysis"));
    let records = usage::read(&usage_path).map_err(|e| e.to_string())?;

    let getenv: delegate::providers::Getenv =
        std::sync::Arc::new(|name: &str| std::env::var(name).ok());
    let build = |entry: &crate::config::delegate::DelegateProviderConfig| {
        delegate::build(entry, getenv.clone())
    };
    let check = |entry: &crate::config::delegate::DelegateProviderConfig, req: &Request| {
        usage::check_budget(entry, req, &records, Utc::now())
    };
    let request = Request {
        prompt,
        max_output_tokens: 0,
        timeout,
        dir: root.display().to_string(),
    };
    let opts = ChainOptions {
        chain: args.chain.clone(),
        confirm: args.confirm,
        max_retry_after: Duration::from_secs(60),
        build: &build,
        check: Some(&check),
        sleep: &delegate::sleep_for,
    };
    let result = delegate::run_chain(cfg, names, &request, &opts);

    let (resp, attempts, run_error) = match result {
        Ok((resp, attempts)) => (Some(resp), attempts, None),
        Err(e) => (None, e.attempts.clone(), Some(e.to_string())),
    };
    let run_path = write_run_record(
        &task_dir,
        file,
        resp.as_ref(),
        &attempts,
        run_error.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    if let Some(err) = run_error {
        let reason = format!("delegate run failed: {err} (see {})", run_path.display());
        tasks::block_task(tasks_dir, file, &reason).map_err(|e| e.to_string())?;
        println!("blocked   {file} — {err}");
        return Ok(());
    }

    let Some(resp) = resp else {
        return Err("a run that answered has no response".to_owned());
    };
    let billable = cfg
        .providers
        .iter()
        .find(|p| p.name == resp.provider)
        .is_some_and(|p| p.billable);
    usage::append(
        &usage_path,
        &usage::record_for(
            &resp.provider,
            &args.chain,
            billable,
            resp.input_tokens,
            resp.output_tokens,
            resp.wall_ms,
        ),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "ran       {file} via {} — review {}, then: plaesy tasks complete {file}",
        resp.provider,
        run_path.display()
    );
    Ok(())
}

/// What the provider receives: the task, and the instruction to report what changed.
fn build_task_prompt(task_file: &str, task_body: &str) -> String {
    format!(
        "You are implementing one task.\n\
         Work in the current directory and change only what the task needs.\n\
         When you finish, reply with a short summary: what changed, and how each acceptance criterion was checked.\n\n\
         ## Task ({task_file})\n\n{task_body}\n"
    )
}

/// Saves a run's output, or its error, under `<task>/runs/`. The file name carries the time.
fn write_run_record(
    task_dir: &Path,
    file: &str,
    resp: Option<&Response>,
    attempts: &[Attempt],
    run_error: Option<&str>,
) -> std::io::Result<PathBuf> {
    let dir = task_dir.join("runs");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{file}-{}.md", Utc::now().format("%H%M%S")));

    let mut text = format!("# Run of {file}\n\n- Time: {}\n", Utc::now().to_rfc3339());
    if let Some(resp) = resp.filter(|r| !r.provider.is_empty()) {
        text.push_str(&format!("- Provider: {}\n", resp.provider));
    }
    match (run_error, resp) {
        (Some(err), _) => text.push_str(&format!("- Result: failed\n\n## Error\n\n{err}\n")),
        (None, Some(resp)) => {
            text.push_str(&format!(
                "- Result: ok — not yet reviewed\n\n## Output\n\n{}\n",
                resp.text
            ));
        }
        (None, None) => text.push_str("- Result: ok — not yet reviewed\n"),
    }
    if !attempts.is_empty() {
        text.push_str(&format!("\n## Attempts\n\n{}\n", attempts.len()));
    }
    fs::write(&path, text)?;
    Ok(path)
}
