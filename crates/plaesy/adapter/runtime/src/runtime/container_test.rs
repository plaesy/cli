use super::*;
use std::process::Stdio;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn sample_spec() -> ContainerSpec {
    ContainerSpec::new("alpine:3.20", "/bin/echo", strings(&["hello"])).with_name("plaesy-test")
}

fn position(args: &[String], value: &str) -> usize {
    args.iter()
        .position(|arg| arg == value)
        .unwrap_or_else(|| panic!("missing {value} in {args:?}"))
}

fn missing_docker() -> PathBuf {
    PathBuf::from("plaesy-no-such-docker-binary")
}

#[test]
fn bounded_runtime_container_argv_is_exact_for_default_spec() {
    let args = sample_spec().docker_args().unwrap();
    assert_eq!(
        args,
        strings(&[
            "run",
            "--rm",
            "--name",
            "plaesy-test",
            "--network",
            "none",
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--pids-limit",
            "128",
            "--memory",
            "256m",
            "--memory-swap",
            "256m",
            "--cpus",
            "1",
            "--user",
            "65534:65534",
            "--tmpfs",
            "/work:rw,nosuid,nodev,noexec,size=64m,mode=1777",
            "--workdir",
            "/work",
            "--entrypoint",
            "/bin/echo",
            "alpine:3.20",
            "hello",
        ])
    );
}

#[test]
fn bounded_runtime_container_isolation_flags_precede_the_image() {
    let args = sample_spec().docker_args().unwrap();
    let image = position(&args, "alpine:3.20");
    for flag in [
        "--network",
        "--read-only",
        "--cap-drop",
        "--security-opt",
        "--pids-limit",
        "--memory",
        "--cpus",
        "--user",
        "--tmpfs",
        "--entrypoint",
    ] {
        assert!(
            position(&args, flag) < image,
            "{flag} must precede the image"
        );
    }
}

#[test]
fn bounded_runtime_container_never_grants_host_access_or_privilege() {
    let args = sample_spec().docker_args().unwrap();
    for forbidden in [
        "--privileged",
        "--volume",
        "-v",
        "--mount",
        "--env",
        "-e",
        "--env-file",
        "--net",
        "--network=host",
        "--pid",
        "--ipc",
        "--cap-add",
        "--device",
        "--add-host",
    ] {
        assert!(
            !args.iter().any(|arg| arg == forbidden),
            "{forbidden} must not appear in {args:?}"
        );
    }
    assert_eq!(args[position(&args, "--network") + 1], "none");
    assert_eq!(args[position(&args, "--cap-drop") + 1], "ALL");
}

#[test]
fn bounded_runtime_container_args_after_image_pass_verbatim_without_shell() {
    let hostile = "a; rm -rf / $(id) `whoami` && echo x";
    let spec = ContainerSpec::new(
        "alpine:3.20",
        "/bin/sh",
        vec!["-c".into(), hostile.into(), "-rf".into()],
    );
    let args = spec.docker_args().unwrap();
    let image = position(&args, "alpine:3.20");
    assert_eq!(&args[image + 1..], &strings(&["-c", hostile, "-rf"])[..]);
}

#[test]
fn bounded_runtime_container_user_must_not_be_root() {
    for user in [
        "0",
        "0:0",
        "root",
        "root:root",
        "0:1000",
        "1000:0",
        "",
        ":1000",
    ] {
        let mut spec = sample_spec();
        spec.user = user.to_owned();
        let error = spec.docker_args().unwrap_err();
        assert_eq!(error.field, "user", "user {user:?} must be rejected");
    }
    for user in ["1000", "1000:1000", "nobody", "65534:65534"] {
        let mut spec = sample_spec();
        spec.user = user.to_owned();
        assert!(spec.docker_args().is_ok(), "user {user:?} must be accepted");
    }
}

#[test]
fn bounded_runtime_container_rejects_invalid_fields() {
    type Mutate = fn(&mut ContainerSpec);
    let cases: &[(&str, Mutate)] = &[
        ("image", |s| s.image = String::new()),
        ("image", |s| s.image = "-privileged".into()),
        ("image", |s| s.image = "alpine;rm".into()),
        ("program", |s| s.program = String::new()),
        ("program", |s| s.program = "a\0b".into()),
        ("name", |s| s.name = Some("-bad".into())),
        ("name", |s| s.name = Some("has space".into())),
        ("pids_limit", |s| s.pids_limit = 0),
        ("memory", |s| s.memory = "0m".into()),
        ("memory", |s| s.memory = "1.5g".into()),
        ("memory", |s| s.memory = "12mm".into()),
        ("memory", |s| s.memory = "m".into()),
        ("cpus", |s| s.cpus = "0".into()),
        ("cpus", |s| s.cpus = "nan".into()),
        ("cpus", |s| s.cpus = "inf".into()),
        ("cpus", |s| s.cpus = "-1".into()),
        ("work_dir", |s| s.work_dir = "work".into()),
        ("work_dir", |s| s.work_dir = "/".into()),
        ("work_dir", |s| s.work_dir = "/w:x".into()),
        ("tmpfs_size", |s| s.tmpfs_size = "0k".into()),
    ];
    for (field, mutate) in cases {
        let mut spec = sample_spec();
        mutate(&mut spec);
        let error = spec
            .docker_args()
            .expect_err(&format!("{field} must be rejected: {spec:?}"));
        assert_eq!(error.field, *field, "wrong field for {spec:?}");
    }
}

#[test]
fn bounded_runtime_container_accepts_documented_limit_forms() {
    for (memory, cpus) in [
        ("1g", "0.5"),
        ("512M", "2"),
        ("2048k", "1.25"),
        ("1024", "1"),
    ] {
        let mut spec = sample_spec();
        spec.memory = memory.into();
        spec.cpus = cpus.into();
        assert!(
            spec.docker_args().is_ok(),
            "memory {memory} and cpus {cpus} must be accepted"
        );
    }
}

#[test]
fn bounded_runtime_container_disabled_by_default() {
    let mut adapter = ContainerizedAdapter::new(
        sample_spec(),
        Duration::from_secs(60),
        vec!["alpine:3.20".into()],
    )
    .with_docker_binary(missing_docker());
    assert_eq!(
        adapter.execute("task-1", &CancelFlag::new()),
        Outcome::Failed {
            reason: "external_effects_disabled".into()
        }
    );
}

#[test]
fn bounded_runtime_container_image_outside_allowlist_is_refused() {
    let mut adapter = ContainerizedAdapter::new(
        sample_spec(),
        Duration::from_secs(60),
        vec!["debian:12".into()],
    )
    .with_external_effects(true)
    .with_docker_binary(missing_docker());
    assert_eq!(
        adapter.execute("task-1", &CancelFlag::new()),
        Outcome::Failed {
            reason: "image_not_allowed".into()
        }
    );
}

#[test]
fn bounded_runtime_container_invalid_spec_fails_before_spawn() {
    let mut spec = sample_spec();
    spec.pids_limit = 0;
    let mut adapter =
        ContainerizedAdapter::new(spec, Duration::from_secs(60), vec!["alpine:3.20".into()])
            .with_external_effects(true)
            .with_docker_binary(missing_docker());
    match adapter.execute("task-1", &CancelFlag::new()) {
        Outcome::Failed { reason } => {
            assert!(
                reason.starts_with("invalid_container_spec: pids_limit"),
                "unexpected reason {reason}"
            );
        }
        other => panic!("expected invalid spec failure, got {other:?}"),
    }
}

#[test]
fn bounded_runtime_container_spawn_failure_is_reported_not_panicked() {
    let mut adapter = ContainerizedAdapter::new(
        sample_spec(),
        Duration::from_secs(60),
        vec!["alpine:3.20".into()],
    )
    .with_external_effects(true)
    .with_docker_binary(missing_docker());
    match adapter.execute("task-1", &CancelFlag::new()) {
        Outcome::Failed { reason } => {
            assert!(
                reason.starts_with("spawn_failed:"),
                "unexpected reason {reason}"
            );
        }
        other => panic!("expected spawn failure, got {other:?}"),
    }
}

#[test]
fn bounded_runtime_container_cancel_before_start_spawns_nothing() {
    let mut adapter = ContainerizedAdapter::new(
        sample_spec(),
        Duration::from_secs(60),
        vec!["alpine:3.20".into()],
    )
    .with_external_effects(true)
    .with_docker_binary(missing_docker());
    let cancel = CancelFlag::new();
    cancel.cancel();
    assert_eq!(adapter.execute("task-1", &cancel), Outcome::Cancelled);
}

fn docker_available() -> bool {
    let ok = Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !ok {
        eprintln!("skipping bounded_runtime_container live test: `docker info` failed or docker is not installed");
    }
    ok
}

fn test_image() -> String {
    std::env::var("PLAESY_TEST_CONTAINER_IMAGE").unwrap_or_else(|_| "alpine:3.20".to_owned())
}

fn live_adapter(script: &str, timeout: Duration) -> ContainerizedAdapter {
    let image = test_image();
    ContainerizedAdapter::new(
        ContainerSpec::new(image.clone(), "sh", strings(&["-c", script])),
        timeout,
        vec![image],
    )
    .with_external_effects(true)
}

#[test]
fn bounded_runtime_container_live_isolation_holds() {
    if !docker_available() {
        return;
    }
    // Exit codes name the failed check: 3 extra network interface, 4 root fs writable,
    // 5 tmpfs work dir not writable, 6 running as root.
    let script = "for n in /sys/class/net/*; do [ \"$(basename \"$n\")\" = lo ] || exit 3; done; \
                  touch /etc/plaesy-probe 2>/dev/null && exit 4; \
                  touch /work/plaesy-probe || exit 5; \
                  [ \"$(id -u)\" != 0 ] || exit 6; \
                  exit 0";
    let mut adapter = live_adapter(script, Duration::from_secs(60));
    match adapter.execute("task-live-1", &CancelFlag::new()) {
        Outcome::Succeeded { .. } => {}
        other => panic!("isolation probe failed: {other:?}"),
    }
}

#[test]
fn bounded_runtime_container_live_timeout_kills_and_removes_container() {
    if !docker_available() {
        return;
    }
    let mut adapter = live_adapter("sleep 60", Duration::from_secs(2));
    assert_eq!(
        adapter.execute("task-live-2", &CancelFlag::new()),
        Outcome::TimedOut
    );
    let prefix = format!("name=plaesy-{}-", std::process::id());
    let remaining = Command::new("docker")
        .args(["ps", "-aq", "--filter", &prefix])
        .output()
        .expect("docker ps must run after docker info succeeded");
    assert!(
        String::from_utf8_lossy(&remaining.stdout).trim().is_empty(),
        "container survived timeout"
    );
}

#[test]
fn bounded_runtime_container_live_cancel_stops_run() {
    if !docker_available() {
        return;
    }
    let mut adapter = live_adapter("sleep 60", Duration::from_secs(120));
    let cancel = CancelFlag::new();
    let trigger = cancel.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        trigger.cancel();
    });
    assert_eq!(adapter.execute("task-live-3", &cancel), Outcome::Cancelled);
    let _ = canceller.join();
}
