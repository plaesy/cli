//! `plaesy init`: initialize project-local Plaesy metadata without changing other files.

use std::fs;
use std::path::PathBuf;

use super::CliResult;

/// Initialize a project's `.plaesy/` directory.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Project directory (defaults to the current directory)
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,
    /// Show what would be created without writing anything
    #[arg(long)]
    pub dry_run: bool,
}

/// Create the project marker expected by `plaesy tasks`.
///
/// Existing content is preserved. This command does not edit Git ignores, Memory settings,
/// or agent instruction files.
pub fn run(args: &Args) -> CliResult {
    let project = args.path.canonicalize()?;
    if !project.is_dir() {
        return Err(format!("project path is not a directory: {}", project.display()).into());
    }
    let mut ancestor = if project.join(".git").exists() {
        None
    } else {
        project.parent()
    };
    while let Some(parent) = ancestor {
        let existing = parent.join(".plaesy");
        match fs::symlink_metadata(&existing) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                return Err(format!(
                    "project is already inside a Plaesy project: {}",
                    existing.display()
                )
                .into());
            }
            Ok(_) => {
                return Err(format!(
                    "existing .plaesy path is not a directory: {}",
                    existing.display()
                )
                .into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        // Match task root discovery: a Git repository is a project boundary.
        if parent.join(".git").exists() {
            break;
        }
        ancestor = parent.parent();
    }

    let plaesy_dir = project.join(".plaesy");
    match fs::symlink_metadata(&plaesy_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            println!(
                "[✓] Plaesy project already initialized: {}",
                plaesy_dir.display()
            );
        }
        Ok(_) => {
            return Err(format!(
                "refusing to replace existing non-directory: {}",
                plaesy_dir.display()
            )
            .into());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if args.dry_run {
                println!("Would create {}", plaesy_dir.display());
            } else {
                fs::create_dir(&plaesy_dir)?;
                println!("[✓] Initialized Plaesy project: {}", plaesy_dir.display());
            }
        }
        Err(error) => return Err(error.into()),
    }
    println!(
        "Tasks will be stored in .plaesy/tasks/. No Git, Memory, or agent files were changed."
    );
    Ok(())
}

#[cfg(test)]
#[path = "init_test.rs"]
mod init_test;
