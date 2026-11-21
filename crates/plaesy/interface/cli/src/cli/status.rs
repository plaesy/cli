//! `plaesy status`: version, git availability, and whether the working directory is a repository.

use super::common::banner;
use super::CliResult;
use crate::common::{git, validate};

pub fn run() -> CliResult {
    banner("Plaesy CLI", &format!("version {}", crate::version()))?;

    if validate::find_command("git").is_some() {
        println!("  ✓ git available");
    } else {
        println!("  ✗ git command not found");
    }

    match git::repo_root() {
        Ok(root) => println!("  ✓ git repository: {}", root.display()),
        Err(_) => println!("  ✗ not inside a git repository"),
    }

    match crate::installer::status(crate::version()) {
        Ok(status) => {
            println!(
                "  {} installed binary: {}",
                if status.installed { "✓" } else { "✗" },
                status.install_path.display()
            );
            println!(
                "  {} install directory on PATH: {}",
                if status.on_path { "✓" } else { "✗" },
                status.install_dir.display()
            );
            if let Some(running) = status.running_from {
                println!("  ✓ running from: {}", running.display());
            }
        }
        Err(error) => println!("  ✗ install status unavailable: {error}"),
    }
    Ok(())
}
