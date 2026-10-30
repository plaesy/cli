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
    banner("Plaesy Constitution Kit", "installer")?;

    let dst =
        installer::install(&log).inspect_err(|e| log.error(&format!("install failed: {e}")))?;
    log.success(&format!("installed to {}", dst.display()));

    if let Ok(home) = installer::plaesy_home_dir() {
        log.info(&format!("Plaesy home ready at {}", home.display()));
    }
    if let Ok(dir) = installer::install_dir() {
        if !installer::on_path(&dir) {
            println!();
            println!("{}", installer::path_instructions(&dir));
        }
    }
    Ok(())
}

pub fn uninstall() -> CliResult {
    let log = logger();
    let path = installer::installed_path()?;
    let app_dir = installer::app_data_dir()?;
    if !path.exists() && !app_dir.exists() {
        println!("plaesy is not installed at {}", path.display());
        return Ok(());
    }

    let dir = installer::install_dir()?;
    let prompt = format!(
        "Remove {}, {} and the PATH entry for {}? [y/N]: ",
        path.display(),
        app_dir.display(),
        dir.display()
    );
    if !installer::confirm(io::stdin().lock(), &prompt) {
        println!("Aborted; nothing was removed.");
        return Ok(());
    }

    let removed =
        installer::uninstall().inspect_err(|e| log.error(&format!("uninstall failed: {e}")))?;
    log.success(&format!(
        "removed {} and {}",
        removed.binary_path.display(),
        app_dir.display()
    ));
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
    let rep = installer::repair(&log).inspect_err(|e| log.error(&format!("repair failed: {e}")))?;
    if rep.binary_copied {
        log.success(&format!("binary restored at {}", rep.binary_path.display()));
    } else {
        log.info(&format!(
            "binary at {} already matches this build",
            rep.binary_path.display()
        ));
    }
    log.success(&format!(
        "Plaesy home refreshed at {}",
        rep.home_dir.display()
    ));
    print_path_hint(rep.on_path)
}

pub fn upgrade() -> CliResult {
    let log = logger();
    let rep = match installer::upgrade(&log) {
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
    log.success(&format!(
        "Plaesy home refreshed at {}",
        rep.home_dir.display()
    ));
    print_path_hint(rep.on_path)
}
