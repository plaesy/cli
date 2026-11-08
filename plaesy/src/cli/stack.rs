//! `plaesy stack`: inspects a target project's technology stack and installs matching instructions.

use std::fs;
use std::path::PathBuf;

use super::CliResult;
use crate::common::git;
use crate::detectstack;
use crate::scaffold::config::instruction_name;

/// Prints the subcommands when none is given.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<StackCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum StackCommand {
    /// List instruction files relevant to a target project's tech stack
    Detect(DetectArgs),
}

#[derive(Debug, clap::Args)]
pub struct DetectArgs {
    /// Project to inspect (default: current directory)
    pub target_dir: Option<PathBuf>,
    /// Path to the plaesy repo root (defaults to $PLAESY_ROOT, then the git repo root)
    #[arg(long = "plaesy-root")]
    pub plaesy_root: Option<PathBuf>,
    /// Copy the detected instruction files into <target>/.plaesy/instructions/ (never overwrites)
    #[arg(long)]
    pub install: bool,
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Some(StackCommand::Detect(d)) => detect(d),
        None => super::print_group_help("stack"),
    }
}

fn detect(args: &DetectArgs) -> CliResult {
    let target = args
        .target_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let root = match args
        .plaesy_root
        .clone()
        .or_else(|| std::env::var_os("PLAESY_ROOT").map(PathBuf::from))
    {
        Some(root) => root,
        None => git::repo_root().map_err(|_| {
            "could not determine PLAESY_ROOT: not a git repository and PLAESY_ROOT is unset"
        })?,
    };

    let files = detectstack::detect(&target, &root)?;
    if !args.install {
        for f in &files {
            println!("{f}");
        }
        return Ok(());
    }

    let dest_dir = target.join(".plaesy").join("instructions");
    fs::create_dir_all(&dest_dir).map_err(|e| format!("creating {}: {e}", dest_dir.display()))?;

    let mut installed = 0usize;
    for f in &files {
        let src = root.join(f);
        let name = instruction_name(f);
        let base = name.strip_suffix(".md").unwrap_or(&name);
        let dst = dest_dir.join(format!("{base}.md"));
        // Never clobber a hand-edited installed file.
        if dst.exists() {
            println!("= {base}.md (exists, kept)");
            continue;
        }
        match fs::read(&src).and_then(|bytes| fs::write(&dst, bytes)) {
            Ok(()) => {
                println!("+ {base}.md");
                installed += 1;
            }
            Err(e) => println!("! {base}.md ({e})"),
        }
    }
    println!(
        "\n{installed} file(s) installed into {}, {} skipped.",
        dest_dir.display(),
        files.len() - installed
    );
    println!(
        "Re-run `plaesy stack detect --install` after adding a dependency that changes the stack."
    );
    Ok(())
}
