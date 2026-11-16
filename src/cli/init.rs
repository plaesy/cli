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
mod tests {
    use super::*;

    #[test]
    fn creates_marker_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let args = Args {
            path: temp.path().to_path_buf(),
            dry_run: false,
        };
        run(&args).unwrap();
        assert!(temp.path().join(".plaesy").is_dir());
        fs::write(temp.path().join(".plaesy/keep.txt"), "keep").unwrap();
        run(&args).unwrap();
        assert_eq!(
            fs::read_to_string(temp.path().join(".plaesy/keep.txt")).unwrap(),
            "keep"
        );
    }

    #[test]
    fn dry_run_does_not_write() {
        let temp = tempfile::tempdir().unwrap();
        run(&Args {
            path: temp.path().to_path_buf(),
            dry_run: true,
        })
        .unwrap();
        assert!(!temp.path().join(".plaesy").exists());
    }

    #[test]
    fn refuses_nested_project_root() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join(".plaesy")).unwrap();
        let nested = temp.path().join("nested");
        fs::create_dir(&nested).unwrap();
        assert!(run(&Args {
            path: nested.clone(),
            dry_run: false
        })
        .is_err());
        assert!(!nested.join(".plaesy").exists());
    }

    #[test]
    fn allows_independent_nested_git_project() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join(".plaesy")).unwrap();
        let nested = temp.path().join("nested");
        fs::create_dir_all(nested.join(".git")).unwrap();
        run(&Args {
            path: nested.clone(),
            dry_run: false,
        })
        .unwrap();
        assert!(nested.join(".plaesy").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_instead_of_treating_it_as_project_directory() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        fs::create_dir(&target).unwrap();
        symlink(&target, temp.path().join(".plaesy")).unwrap();
        assert!(run(&Args {
            path: temp.path().to_path_buf(),
            dry_run: false
        })
        .is_err());
        assert!(fs::read_dir(&target).unwrap().next().is_none());
    }
}
