use std::process::Command;
use tempfile::TempDir;

#[test]
fn sessions_list_runs_outside_a_plaesy_project() {
    let outside_project = TempDir::new().expect("create outside-project directory");
    let output = Command::new(env!("CARGO_BIN_EXE_plaesy"))
        .args(["sessions", "list"])
        .current_dir(outside_project.path())
        .output()
        .expect("run plaesy sessions list");

    assert!(
        output.status.success(),
        "sessions list failed outside a project: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Session state is self-reported"));
}
