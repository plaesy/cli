//! Container isolation for the native runtime (release gate 1).
//!
//! [`ContainerSpec`] builds the argument vector for `docker run`. Every value is a
//! separate element, so nothing is shell-interpreted. The run gets no network, a
//! read-only root filesystem, no Linux capabilities, a non-root user, a tmpfs work
//! directory, and pid, memory, and CPU limits.
//!
//! [`ContainerizedAdapter`] runs that vector under the same bounded policy as
//! [`NativeProcessAdapter`](super::native::NativeProcessAdapter). Its non-docker tests
//! cover the argument list and the refusal paths. Live isolation is covered only by
//! docker-dependent tests, which skip when `docker info` fails.

use super::native::run_bounded;
use crate::adapter::{CancelFlag, ExecutionAdapter, Outcome};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_USER: &str = "65534:65534";
const DEFAULT_WORK_DIR: &str = "/work";
const DEFAULT_TMPFS_SIZE: &str = "64m";
const DEFAULT_MEMORY: &str = "256m";
const DEFAULT_CPUS: &str = "1";
const DEFAULT_PIDS_LIMIT: u32 = 128;
const REMOVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Environment variables copied to the docker client. The container receives none.
const DOCKER_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "HOME",
    "DOCKER_HOST",
    "DOCKER_CONFIG",
];

static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A rejected container field. `field` names the spec field and `reason` says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSpecError {
    /// Name of the rejected field.
    pub field: &'static str,
    /// Short reason, safe to log.
    pub reason: &'static str,
}

impl fmt::Display for ContainerSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.field, self.reason)
    }
}

impl std::error::Error for ContainerSpecError {}

/// Isolation and resource settings for one `docker run`.
///
/// The `image`, `program`, and `args` fields select what runs. `program` becomes the
/// container entrypoint and `args` its argv. The remaining fields have safe defaults
/// from [`ContainerSpec::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSpec {
    /// Image reference, for example `alpine:3.20`.
    pub image: String,
    /// Program to run inside the container.
    pub program: String,
    /// Arguments passed to `program`, one element each.
    pub args: Vec<String>,
    /// Container name. The adapter sets this for each run so it can remove the container.
    pub name: Option<String>,
    /// `uid[:gid]` for the container process. Must not be root.
    pub user: String,
    /// Maximum number of processes and threads.
    pub pids_limit: u32,
    /// Memory limit, in docker size syntax such as `256m`. Swap is disabled.
    pub memory: String,
    /// CPU limit, as a decimal number of CPUs such as `0.5`.
    pub cpus: String,
    /// Absolute path of the tmpfs work directory.
    pub work_dir: String,
    /// Size of the tmpfs work directory, in docker size syntax.
    pub tmpfs_size: String,
}

impl ContainerSpec {
    /// Creates a spec with safe defaults: non-root user, no network, and bounded resources.
    #[must_use]
    pub fn new(image: impl Into<String>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            image: image.into(),
            program: program.into(),
            args,
            name: None,
            user: DEFAULT_USER.to_owned(),
            pids_limit: DEFAULT_PIDS_LIMIT,
            memory: DEFAULT_MEMORY.to_owned(),
            cpus: DEFAULT_CPUS.to_owned(),
            work_dir: DEFAULT_WORK_DIR.to_owned(),
            tmpfs_size: DEFAULT_TMPFS_SIZE.to_owned(),
        }
    }

    /// Sets the container name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Builds the argument vector for `docker run`, without the `docker` program name.
    ///
    /// # Errors
    ///
    /// Returns [`ContainerSpecError`] when a field is empty, malformed, or would run as
    /// root. No arguments are produced in that case.
    pub fn docker_args(&self) -> Result<Vec<String>, ContainerSpecError> {
        self.validate()?;

        let mut argv: Vec<String> = vec!["run".into(), "--rm".into()];
        if let Some(name) = &self.name {
            push_flag(&mut argv, "--name", name.clone());
        }
        push_flag(&mut argv, "--network", "none");
        argv.push("--read-only".into());
        push_flag(&mut argv, "--cap-drop", "ALL");
        push_flag(&mut argv, "--security-opt", "no-new-privileges");
        push_flag(&mut argv, "--pids-limit", self.pids_limit.to_string());
        push_flag(&mut argv, "--memory", self.memory.clone());
        push_flag(&mut argv, "--memory-swap", self.memory.clone());
        push_flag(&mut argv, "--cpus", self.cpus.clone());
        push_flag(&mut argv, "--user", self.user.clone());
        push_flag(
            &mut argv,
            "--tmpfs",
            format!(
                "{}:rw,nosuid,nodev,noexec,size={},mode=1777",
                self.work_dir, self.tmpfs_size
            ),
        );
        push_flag(&mut argv, "--workdir", self.work_dir.clone());
        push_flag(&mut argv, "--entrypoint", self.program.clone());
        argv.push(self.image.clone());
        argv.extend(self.args.iter().cloned());
        Ok(argv)
    }

    fn validate(&self) -> Result<(), ContainerSpecError> {
        let invalid = |field, reason| Err(ContainerSpecError { field, reason });

        if self.image.is_empty() {
            return invalid("image", "must not be empty");
        }
        if self.image.starts_with('-') {
            return invalid("image", "must not start with '-'");
        }
        if !self.image.chars().all(is_image_char) {
            return invalid(
                "image",
                "contains characters outside the image reference set",
            );
        }
        if self.program.is_empty() {
            return invalid("program", "must not be empty");
        }
        if self.program.contains('\0') || self.args.iter().any(|arg| arg.contains('\0')) {
            return invalid("program", "must not contain NUL bytes");
        }
        if let Some(name) = &self.name {
            if !is_container_name(name) {
                return invalid("name", "must match [a-zA-Z0-9][a-zA-Z0-9_.-]*");
            }
        }
        if !is_non_root_user(&self.user) {
            return invalid("user", "must be a non-root uid[:gid]");
        }
        if self.pids_limit == 0 {
            return invalid("pids_limit", "must be greater than zero");
        }
        if !is_positive_size(&self.memory) {
            return invalid("memory", "must be a positive size such as 256m");
        }
        if !self
            .cpus
            .parse::<f64>()
            .is_ok_and(|cpus| cpus.is_finite() && cpus > 0.0)
        {
            return invalid("cpus", "must be a positive decimal such as 0.5");
        }
        if !self.work_dir.starts_with('/')
            || self.work_dir.len() < 2
            || self.work_dir.contains([',', ':', '\0'])
        {
            return invalid("work_dir", "must be an absolute path without ',' or ':'");
        }
        if !is_positive_size(&self.tmpfs_size) {
            return invalid("tmpfs_size", "must be a positive size such as 64m");
        }
        Ok(())
    }
}

fn push_flag(argv: &mut Vec<String>, flag: &str, value: impl Into<String>) {
    argv.push(flag.to_owned());
    argv.push(value.into());
}

fn is_image_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@')
}

fn is_container_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn is_non_root_user(user: &str) -> bool {
    let mut parts = user.split(':');
    let uid = parts.next().unwrap_or_default();
    if uid.is_empty() || uid == "0" || uid == "root" {
        return false;
    }
    parts.all(|part| !part.is_empty() && part != "0" && part != "root")
}

/// Accepts a positive integer with an optional `b`, `k`, `m`, or `g` suffix.
fn is_positive_size(value: &str) -> bool {
    let digits = value.trim_end_matches(['b', 'B', 'k', 'K', 'm', 'M', 'g', 'G']);
    let suffix_len = value.len() - digits.len();
    suffix_len <= 1
        && !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && digits.parse::<u64>().is_ok_and(|number| number > 0)
}

fn next_run_name() -> String {
    let counter = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("plaesy-{}-{counter}-{stamp}", std::process::id())
}

fn apply_docker_env(command: &mut Command) {
    for name in DOCKER_ENV_ALLOWLIST {
        if let Ok(value) = std::env::var(name) {
            command.env(name, value);
        }
    }
}

/// Force-removes a container by name. Used after a timeout or cancel, because killing
/// the `docker run` client does not stop the container itself.
fn remove_container(docker: &Path, name: &str) {
    let mut command = Command::new(docker);
    command.args(["rm", "-f", name]).env_clear();
    apply_docker_env(&mut command);
    let _ = run_bounded(command, REMOVE_TIMEOUT, &CancelFlag::new(), 0);
}

/// Runs one program inside a locked-down container through the `docker` CLI.
#[derive(Debug, Clone)]
pub struct ContainerizedAdapter {
    spec: ContainerSpec,
    docker: PathBuf,
    timeout: Duration,
    max_output_bytes: usize,
    allowed_images: Vec<String>,
    external_effects_enabled: bool,
}

impl ContainerizedAdapter {
    /// Creates an adapter for `spec`. External effects are off until enabled.
    /// Only images listed in `allowed_images` may run.
    #[must_use]
    pub fn new(spec: ContainerSpec, timeout: Duration, allowed_images: Vec<String>) -> Self {
        Self {
            spec,
            docker: PathBuf::from("docker"),
            timeout,
            max_output_bytes: 64 * 1024,
            allowed_images,
            external_effects_enabled: false,
        }
    }

    /// Turns external effects on or off for this deployment.
    #[must_use]
    pub fn with_external_effects(mut self, enabled: bool) -> Self {
        self.external_effects_enabled = enabled;
        self
    }

    /// Sets the docker client binary. The default is `docker` from the search path.
    #[must_use]
    pub fn with_docker_binary(mut self, docker: impl Into<PathBuf>) -> Self {
        self.docker = docker.into();
        self
    }

    /// Sets the per-stream output cap in bytes.
    #[must_use]
    pub fn with_max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = max;
        self
    }
}

impl ExecutionAdapter for ContainerizedAdapter {
    fn name(&self) -> &str {
        "containerized-docker"
    }

    fn execute(&mut self, _task_id: &str, cancel: &CancelFlag) -> Outcome {
        if cancel.is_cancelled() {
            return Outcome::Cancelled;
        }
        if !self.external_effects_enabled {
            return Outcome::Failed {
                reason: "external_effects_disabled".to_owned(),
            };
        }
        if !self
            .allowed_images
            .iter()
            .any(|image| image == &self.spec.image)
        {
            return Outcome::Failed {
                reason: "image_not_allowed".to_owned(),
            };
        }

        let name = next_run_name();
        let args = match self.spec.clone().with_name(name.clone()).docker_args() {
            Ok(args) => args,
            Err(error) => {
                return Outcome::Failed {
                    reason: format!("invalid_container_spec: {error}"),
                };
            }
        };

        let mut command = Command::new(&self.docker);
        command.args(&args).env_clear();
        apply_docker_env(&mut command);

        let outcome = run_bounded(command, self.timeout, cancel, self.max_output_bytes);
        if matches!(outcome, Outcome::TimedOut | Outcome::Cancelled) {
            remove_container(&self.docker, &name);
        }
        outcome
    }
}

#[cfg(test)]
#[path = "container_test.rs"]
mod container_test;
