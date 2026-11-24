//! `plaesy install`, `uninstall`, `repair`, and `upgrade`: manage the installed binary.

use std::io;
use std::path::PathBuf;

use super::common::{banner, logger};
use super::CliResult;
use crate::installer::update::{self, UpdateStatus};
use crate::installer::{self, InstallerError};

/// Prints the running version, the running binary path, and the installed binary path.
pub fn version() -> CliResult {
    println!("plaesy {}", crate::version());
    println!("binary: {}", installer::running_binary()?.display());
    match installer::installed_path() {
        Ok(path) if path.is_file() => println!("installed: {}", path.display()),
        Ok(path) => println!("installed: not installed (expected {})", path.display()),
        Err(error) => println!("installed: unknown ({error})"),
    }
    Ok(())
}

/// `plaesy update --check`: compares the installed version with a local manifest. Offline only.
#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    /// Compare the installed version with the manifest. Required, because downloading releases
    /// is not implemented.
    #[arg(long)]
    pub check: bool,
    /// Local JSON manifest whose `version` field names the latest release
    #[arg(long, value_name = "PATH")]
    pub manifest: Option<PathBuf>,
}

pub fn update(args: &UpdateArgs) -> CliResult {
    if !args.check {
        return Err(
            "plaesy update needs --check; downloading a release is not implemented and the runtime stays offline"
                .into(),
        );
    }
    let Some(manifest) = args.manifest.as_deref() else {
        return Err(
            "--check needs --manifest <PATH> pointing to a local JSON file with a \"version\" field"
                .into(),
        );
    };
    let latest = update::manifest_version(manifest)?;
    match update::installed_version()? {
        None => {
            println!("installed: not installed; run `plaesy install` first");
            println!("latest:    {latest} ({})", manifest.display());
        }
        Some(installed) => {
            println!("installed: {installed}");
            println!("latest:    {latest} ({})", manifest.display());
            match update::compare(installed, latest) {
                UpdateStatus::UpToDate => println!("up to date"),
                UpdateStatus::UpdateAvailable => {
                    println!("update available: build {latest} and run its `plaesy upgrade`")
                }
                UpdateStatus::InstalledIsNewer => {
                    println!("installed {installed} is newer than the manifest {latest}");
                }
            }
        }
    }
    Ok(())
}

/// Prints the PATH hint when the install directory is not on PATH.
fn print_path_hint(on_path: bool) -> CliResult {
    if on_path {
        return Ok(());
    }
    let dir = installer::install_dir()?;
    println!();
    println!("{}", installer::path_instructions(&dir));
    Ok(())
}

pub fn install() -> CliResult {
    let log = logger();
    banner("Plaesy CLI", "installer")?;

    let dst = installer::install().inspect_err(|e| log.error(&format!("install failed: {e}")))?;
    log.success(&format!("installed to {}", dst.display()));

    if let Ok(dir) = installer::install_dir() {
        match installer::add_to_user_path(&dir) {
            Ok(true) => {
                log.success(&format!(
                    "added {} to your user PATH; open a new terminal to use `plaesy`",
                    dir.display()
                ));
            }
            Ok(false) if installer::on_path(&dir) => {
                log.info(&format!("{} is already available on PATH", dir.display()));
            }
            Ok(false) => {
                log.success(&format!(
                    "{} is configured on your user PATH; open a new terminal to use `plaesy`",
                    dir.display()
                ));
            }
            Err(error) => {
                log.warning(&format!("could not update PATH automatically: {error}"));
                println!();
                println!("{}", installer::path_instructions(&dir));
            }
        }
    }
    Ok(())
}

pub fn uninstall() -> CliResult {
    let log = logger();
    let path = installer::installed_path()?;
    let binary_exists = path.exists();
    let dir = installer::install_dir()?;
    let prompt = if binary_exists {
        format!(
            "Remove {} and the PATH entry for {}? User data will be kept. [y/N]: ",
            path.display(),
            dir.display()
        )
    } else {
        format!(
            "{} is already absent. Remove a stale PATH entry for {}? [y/N]: ",
            path.display(),
            dir.display()
        )
    };
    if !installer::confirm(io::stdin().lock(), &prompt) {
        println!("Aborted; nothing was removed.");
        return Ok(());
    }

    let removed =
        installer::uninstall().inspect_err(|e| log.error(&format!("uninstall failed: {e}")))?;
    if binary_exists {
        log.success(&format!("removed {}", removed.binary_path.display()));
    } else {
        log.info(&format!(
            "{} was already absent",
            removed.binary_path.display()
        ));
    }
    if removed.path_changed {
        log.success(&format!(
            "removed {} from PATH; open a new terminal",
            removed.path_dir.display()
        ));
    } else {
        log.info(&format!(
            "no PATH entry for {} was found",
            removed.path_dir.display()
        ));
    }
    Ok(())
}

pub fn repair() -> CliResult {
    let log = logger();
    let rep = installer::repair().inspect_err(|e| log.error(&format!("repair failed: {e}")))?;
    if rep.binary_copied {
        log.success(&format!("binary restored at {}", rep.binary_path.display()));
    } else {
        log.info(&format!(
            "binary at {} already matches this build",
            rep.binary_path.display()
        ));
    }
    print_path_hint(rep.on_path)
}

pub fn upgrade() -> CliResult {
    let log = logger();
    let rep = match installer::upgrade() {
        Ok(rep) => rep,
        Err(InstallerError::RunningFromInstall) => {
            log.info(&InstallerError::RunningFromInstall.to_string());
            return Ok(());
        }
        Err(e) => {
            log.error(&format!("upgrade failed: {e}"));
            return Err(e.into());
        }
    };
    if rep.binary_copied {
        log.success(&format!(
            "upgraded {} to this build (version {})",
            rep.binary_path.display(),
            crate::version()
        ));
    } else {
        log.info(&format!(
            "already up to date: {} matches this build (version {})",
            rep.binary_path.display(),
            crate::version()
        ));
    }
    print_path_hint(rep.on_path)
}
