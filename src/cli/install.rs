//! `plaesy install`, `uninstall`, `repair`, and `upgrade`: manage the installed binary.

use std::io;

use super::common::{banner, logger};
use super::CliResult;
use crate::installer::{self, InstallerError};

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
