//! Commands started in the background. Output goes to `<dir>/<name>.out`, and the
//! exit code is written to `<dir>/<name>.exit` when the command finishes.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Child;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// A running job. Wait for it, then read its result.
pub struct Job {
    #[cfg(unix)]
    pid: u32,
    finished: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    #[cfg(windows)]
    job_handle: windows_sys::Win32::Foundation::HANDLE,
}

/// Outcome from waiting for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitOutcome {
    Finished,
    TimedOut,
}

impl Job {
    /// Blocks until the command has finished and its exit file is written.
    pub fn wait(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Waits up to `timeout`, terminating the whole process group on expiry.
    /// `None` waits without a deadline; `Some(Duration::ZERO)` expires immediately.
    ///
    /// # Errors
    ///
    /// An error if the child status cannot be polled or its process group cannot be terminated.
    pub fn wait_timeout(mut self, timeout: Option<Duration>) -> io::Result<WaitOutcome> {
        let started = Instant::now();
        loop {
            if self.finished.load(Ordering::Acquire) {
                self.join_waiter();
                return Ok(WaitOutcome::Finished);
            }
            if timeout.is_some_and(|limit| started.elapsed() >= limit) {
                #[cfg(unix)]
                terminate_process_tree(self.pid)?;
                #[cfg(windows)]
                terminate_process_tree(self.job_handle)?;
                #[cfg(not(any(unix, windows)))]
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "process-tree termination is unsupported",
                ));
                self.join_waiter();
                return Ok(WaitOutcome::TimedOut);
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn join_waiter(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // Closing a kill-on-close job releases any descendants the command left behind.
        // SAFETY: this handle is created once for this Job and closed once here.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job_handle) };
    }
}

#[cfg(unix)]
pub(crate) fn terminate_process_tree(pid: u32) -> io::Result<()> {
    // SAFETY: start() places the child in a process group whose id is its pid.
    let result = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
    if result == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn terminate_process_tree(
    job: windows_sys::Win32::Foundation::HANDLE,
) -> io::Result<()> {
    // SAFETY: `job` is retained for the lifetime of the launched process.
    let result = unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(job, 1) };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(_pid: u32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "process-tree termination is unsupported",
    ))
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

#[cfg(unix)]
pub(crate) fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
pub(crate) fn configure_suspended_start(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_SUSPENDED);
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
    #[cfg(unix)]
    configure_process_group(&mut command);
    #[cfg(windows)]
    configure_suspended_start(&mut command);
    let mut child = command.spawn()?;
    #[cfg(unix)]
    let pid = child.id();
    #[cfg(windows)]
    let job_handle = assign_to_kill_on_close_job(&mut child)?;
    #[cfg(windows)]
    if let Err(error) = resume_suspended_child(&child) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(job_handle) };
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = Arc::clone(&finished);
    let handle = thread::spawn(move || {
        let code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(-1),
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(_) => break -1,
            }
        };
        let tmp = exit_path.with_extension("exit.tmp");
        if fs::write(&tmp, format!("{code}\n")).is_ok() {
            let _ = fs::rename(&tmp, &exit_path);
        }
        worker_finished.store(true, Ordering::Release);
    });
    Ok(Job {
        #[cfg(unix)]
        pid,
        finished,
        handle: Some(handle),
        #[cfg(windows)]
        job_handle,
    })
}

#[cfg(windows)]
pub(crate) fn assign_to_kill_on_close_job(
    child: &mut Child,
) -> io::Result<windows_sys::Win32::Foundation::HANDLE> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    // SAFETY: handles are checked and the structure is initialized before passing to Win32.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        ) == 0
        {
            windows_sys::Win32::Foundation::CloseHandle(job);
            return Err(io::Error::last_os_error());
        }
        let process = child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
        if AssignProcessToJobObject(job, process) == 0 {
            windows_sys::Win32::Foundation::CloseHandle(job);
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }
}

#[cfg(windows)]
pub(crate) fn resume_suspended_child(child: &Child) -> io::Result<()> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    // The process has not run yet, so its primary thread is the only thread to resume.
    // SAFETY: all Win32 handles are checked and closed; the entry has its required size.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut found = Thread32First(snapshot, &mut entry) != 0;
        while found {
            if entry.th32OwnerProcessID == child.id() {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if thread.is_null() {
                    let error = io::Error::last_os_error();
                    windows_sys::Win32::Foundation::CloseHandle(snapshot);
                    return Err(error);
                }
                let result = ResumeThread(thread);
                windows_sys::Win32::Foundation::CloseHandle(thread);
                windows_sys::Win32::Foundation::CloseHandle(snapshot);
                return if result == u32::MAX {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                };
            }
            found = Thread32Next(snapshot, &mut entry) != 0;
        }
        windows_sys::Win32::Foundation::CloseHandle(snapshot);
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "could not find the suspended child process thread",
        ))
    }
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
