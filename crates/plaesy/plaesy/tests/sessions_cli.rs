use std::process::Command;
use tempfile::TempDir;

#[test]
fn tasks_complete_warns_that_manual_completion_is_unverified() {
    let project = TempDir::new().expect("create temporary project");
    // `.git` marks the temp dir as its own project boundary, so `init` does not refuse because
    // a parent directory (for example the home dir) already contains a `.plaesy`.
    std::fs::create_dir(project.path().join(".git")).expect("mark project root");
    let binary = env!("CARGO_BIN_EXE_plaesy");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .current_dir(project.path())
            .output()
            .expect("run plaesy CLI")
    };

    assert!(run(&["init"]).status.success());
    let created = run(&[
        "tasks",
        "--spec",
        "complete-test",
        "create",
        "high",
        "qa",
        "Manual completion",
    ]);
    assert!(
        created.status.success(),
        "task creation failed: status {:?}\nstdout:\n{}\nstderr:\n{}",
        created.status,
        String::from_utf8_lossy(&created.stdout),
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(run(&["tasks", "--spec", "complete-test", "next"])
        .status
        .success());
    let task = "high_qa-manual-completion";
    assert!(run(&["tasks", "--spec", "complete-test", "start", task])
        .status
        .success());
    // `complete` is the reviewer step: the task must reach `review` first.
    assert!(run(&["tasks", "move", task, "doing", "review"])
        .status
        .success());

    let output = run(&["tasks", "--spec", "complete-test", "complete", task]);
    assert!(
        output.status.success(),
        "tasks complete failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Manual completion only; checklist and goal were not verified."));
    assert!(project
        .path()
        .join(".plaesy/tasks/done")
        .join(task)
        .is_dir());
}

#[cfg(unix)]
#[test]
fn tasks_list_all_does_not_print_goal_from_symlinked_task_file() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let project = TempDir::new().expect("create temporary project");
    // `.git` marks the temp dir as its own project boundary, so `init` does not refuse because
    // a parent directory (for example the home dir) already contains a `.plaesy`.
    std::fs::create_dir(project.path().join(".git")).expect("mark project root");
    let binary = env!("CARGO_BIN_EXE_plaesy");
    let run = |args: &[&str]| {
        Command::new(binary)
            .args(args)
            .current_dir(project.path())
            .output()
            .expect("run plaesy CLI")
    };

    assert!(run(&["init"]).status.success());
    let created = run(&[
        "tasks",
        "--spec",
        "list-symlink-test",
        "create",
        "high",
        "qa",
        "Board symlink",
    ]);
    assert!(
        created.status.success(),
        "task creation failed: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task_dir = project
        .path()
        .join(".plaesy/tasks/backlog/high_qa-board-symlink");
    let task_file = task_dir.join("task.md");
    fs::remove_file(&task_file).expect("remove original task file");

    let external = project.path().join("external-task.md");
    fs::write(
        &external,
        "---\\nid: high_qa-board-symlink\\ntitle: Board symlink\\n---\\n\\n## Goal\\nSYNTHETIC_EXTERNAL_GOAL\\n",
    )
    .expect("write synthetic external task");
    symlink(&external, &task_file).expect("symlink synthetic external task");

    let output = run(&["tasks", "--spec", "list-symlink-test", "list", "--all"]);
    assert!(
        !output.status.success(),
        "listing should reject the symlinked task file"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("task file is a symlink"),
        "unexpected listing error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("SYNTHETIC_EXTERNAL_GOAL"),
        "board printed content from the symlink target"
    );
    assert_eq!(
        fs::read_to_string(&external).expect("read synthetic external task"),
        "---\\nid: high_qa-board-symlink\\ntitle: Board symlink\\n---\\n\\n## Goal\\nSYNTHETIC_EXTERNAL_GOAL\\n"
    );
}

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
