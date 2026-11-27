//! `plaesy triage`: a local diagnostics bundle for a bug report. Nothing is sent. The bundle goes to
//! stdout, or to a file with `--out`, and every line passes through the secret redactor first.

use std::path::{Path, PathBuf};
use std::process::Command;

use plaesy_domain_identity::secrets::redact_text;

use super::common::find_root;
use super::delegate::{delegate_status, load_delegate_config};
use super::CliResult;

/// Writes a redacted diagnostics bundle. Nothing is posted anywhere.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Write the bundle to this file instead of stdout
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// Facts collected from the machine, before redaction.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub git: Option<String>,
    pub in_repo: bool,
    /// `(name, kind, status)` for each delegate provider.
    pub providers: Vec<(String, String, String)>,
    /// Problems met while collecting, such as an unreadable delegate config.
    pub notes: Vec<String>,
}

/// Renders the facts as plain text, without redaction.
#[must_use]
pub fn render(facts: &Facts) -> String {
    let mut out = String::new();
    out.push_str("# plaesy triage bundle\n");
    out.push_str(&format!("plaesy: {}\n", facts.version));
    out.push_str(&format!("os: {} ({})\n", facts.os, facts.arch));
    out.push_str(&format!(
        "git: {}\n",
        facts.git.as_deref().unwrap_or("not found")
    ));
    out.push_str(&format!(
        "repository: {}\n",
        if facts.in_repo { "yes" } else { "no" }
    ));
    out.push_str("delegate providers:\n");
    if facts.providers.is_empty() {
        out.push_str("  (none)\n");
    }
    for (name, kind, status) in &facts.providers {
        out.push_str(&format!("  {name} kind={kind} status={status}\n"));
    }
    for note in &facts.notes {
        out.push_str(&format!("note: {note}\n"));
    }
    out
}

/// Renders the bundle and removes secrets from it. Every output path goes through here.
#[must_use]
pub fn bundle(facts: &Facts) -> String {
    redact_text(&render(facts))
}

fn collect() -> Facts {
    let mut facts = Facts {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        ..Facts::default()
    };
    facts.git = Command::new("git")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    facts.in_repo = find_root().is_ok();
    match load_delegate_config(Path::new(".")) {
        Ok(cfg) => {
            for p in &cfg.providers {
                facts.providers.push((
                    p.name.to_string(),
                    p.kind.to_string(),
                    delegate_status(p).to_owned(),
                ));
            }
        }
        Err(e) => facts
            .notes
            .push(format!("could not read delegate config: {e}")),
    }
    facts
}

pub fn run(args: &Args) -> CliResult {
    let text = bundle(&collect());
    match &args.out {
        Some(path) => {
            std::fs::write(path, text)?;
            eprintln!("wrote {}", path.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}

#[cfg(test)]
#[path = "triage_test.rs"]
mod triage_test;
