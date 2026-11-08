//! `plaesy status`: version, git availability, and whether the working directory is a repository.

use super::common::banner;
use super::CliResult;
use crate::common::{git, validate};

pub fn run() -> CliResult {
    banner(
        "Plaesy Constitution Kit",
        &format!("version {}", crate::version()),
    )?;

    if validate::find_command("git").is_some() {
        println!("  ✓ git available");
    } else {
        println!("  ✗ git command not found");
    }

    match git::repo_root() {
        Ok(root) => println!("  ✓ git repository: {}", root.display()),
        Err(_) => println!("  ✗ not inside a git repository"),
    }
    Ok(())
}
