//! Output helpers shared by the commands: the log lines, and the banner.

use std::io::{self, Write};

use crate::common::log::Logger;

/// The logger every command uses.
#[must_use]
pub fn logger() -> Logger {
    Logger::new("plaesy")
}

/// The project root above the working directory (the folder that holds `.plaesy/`).
///
/// # Errors
///
/// The working directory cannot be read, or no project root is found above it.
pub fn find_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    Ok(crate::tasks::find_project_root(&cwd)?)
}

/// The bash that runs contract commands. On Windows, a bare `bash` resolves through
/// `CreateProcess`, which searches `C:\Windows\System32` before `PATH`. That finds the WSL
/// launcher, which runs commands in a different filesystem, so prefer Git for Windows' bash.
pub fn bash_program() -> std::path::PathBuf {
    if cfg!(windows) {
        for candidate in [
            r"C:\Program Files\Git\bin\bash.exe",
            r"C:\Program Files\Git\usr\bin\bash.exe",
        ] {
            let path = std::path::PathBuf::from(candidate);
            if path.is_file() {
                return path;
            }
        }
    }
    std::path::PathBuf::from("bash")
}

/// Runs `command` with `bash -c` in `root`. Returns the exit status and the standard output.
/// A command that cannot start is an error; a non-zero exit is not.
///
/// # Errors
///
/// `bash` cannot be started.
pub fn run_bash(root: &std::path::Path, command: &str) -> Result<(i32, String), String> {
    let out = std::process::Command::new(bash_program())
        .arg("-c")
        .arg(command)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    Ok((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    ))
}

/// Prints the framework banner to standard output.
///
/// # Errors
///
/// Writing to standard output fails.
pub fn banner(title: &str, subtitle: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    crate::common::banner::write_banner(&mut out, title, subtitle)?;
    out.flush()
}
