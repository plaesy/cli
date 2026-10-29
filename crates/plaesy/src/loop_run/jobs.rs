//! Commands started in the background. Output goes to `<dir>/<name>.out`, and the
//! exit code is written to `<dir>/<name>.exit` when the command finishes.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::{self, JoinHandle};

/// A running job. Wait for it, then read its result.
pub struct Job {
    handle: Option<JoinHandle<()>>,
}

impl Job {
    /// Blocks until the command has finished and its exit file is written.
    pub fn wait(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// What a step reads back from a job's files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JobResult {
    /// False while the job has not written its exit file.
    pub finished: bool,
    pub exit_code: i32,
    /// Output written so far.
    pub output: String,
}

fn job_file(dir: &Path, name: &str, ext: &str) -> PathBuf {
    dir.join(format!("{name}{ext}"))
}

/// Starts `argv` in the background with `work_dir` as its directory, and returns at once.
///
/// # Errors
///
/// [`io::ErrorKind::InvalidInput`] for an empty command, or I/O and spawn failures.
pub fn start(work_dir: Option<&Path>, dir: &Path, name: &str, argv: &[String]) -> io::Result<Job> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty job command"))?;
    fs::create_dir_all(dir)?;
    let exit_path = job_file(dir, name, ".exit");
    let _ = fs::remove_file(&exit_path);
    let out = File::create(job_file(dir, name, ".out"))?;
    let err = out.try_clone()?;

    let mut command = Command::new(program);
    command.args(args).stdout(out).stderr(err);
    if let Some(work_dir) = work_dir {
        command.current_dir(work_dir);
    }
    let mut child = command.spawn()?;
    let handle = thread::spawn(move || {
        let code = match child.wait() {
            Ok(status) => status.code().unwrap_or(-1),
            Err(_) => -1,
        };
        let tmp = exit_path.with_extension("exit.tmp");
        if fs::write(&tmp, format!("{code}\n")).is_ok() {
            let _ = fs::rename(&tmp, &exit_path);
        }
    });
    Ok(Job {
        handle: Some(handle),
    })
}

/// Reads a job's result from its files. An unfinished job is not an error.
///
/// # Errors
///
/// I/O failures other than a missing file, and an exit file that is not a number.
pub fn read_result(dir: &Path, name: &str) -> io::Result<JobResult> {
    let output = match fs::read_to_string(job_file(dir, name, ".out")) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let exit = match fs::read_to_string(job_file(dir, name, ".exit")) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(JobResult {
                finished: false,
                exit_code: 0,
                output,
            })
        }
        Err(e) => return Err(e),
    };
    let exit_code = exit
        .trim()
        .parse::<i32>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(JobResult {
        finished: true,
        exit_code,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_job_has_its_exit_code_and_output() {
        let dir = tempfile::tempdir().unwrap();
        let argv = if cfg!(windows) {
            vec!["cmd".to_owned(), "/C".into(), "echo hello".into()]
        } else {
            vec!["sh".to_owned(), "-c".into(), "echo hello".into()]
        };
        let job = start(None, dir.path(), "step-1", &argv).unwrap();
        job.wait();
        let result = read_result(dir.path(), "step-1").unwrap();
        assert!(result.finished);
        assert_eq!(result.exit_code, 0);
        assert!(result.output.contains("hello"));
    }

    #[test]
    fn a_job_with_no_exit_file_is_unfinished_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!read_result(dir.path(), "never").unwrap().finished);
    }
}
