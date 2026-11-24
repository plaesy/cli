use super::*;

fn write_task(root: &Path, status: &str, id: &str, body: &str) {
    let dir = root.join(".plaesy").join("tasks").join(status).join(id);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("task.md"), body).expect("write task.md");
}

#[test]
fn every_moving_action_names_its_target_status_and_show_moves_nothing() {
    assert_eq!(target_status(Action::Next), Some("todo"));
    assert_eq!(target_status(Action::Start), Some("doing"));
    assert_eq!(target_status(Action::Block), Some("blocked"));
    assert_eq!(target_status(Action::Unblock), Some("todo"));
    assert_eq!(target_status(Action::Complete), Some("done"));
    assert_eq!(target_status(Action::Show), None);
}

#[test]
fn actions_follow_the_status_and_show_is_always_offered() {
    assert_eq!(actions_for("backlog"), vec![Action::Next, Action::Show]);
    assert_eq!(actions_for("todo"), vec![Action::Start, Action::Show]);
    assert_eq!(actions_for("doing"), vec![Action::Block, Action::Show]);
    assert_eq!(actions_for("blocked"), vec![Action::Unblock, Action::Show]);
    assert_eq!(actions_for("review"), vec![Action::Complete, Action::Show]);
    assert_eq!(
        actions_for("done"),
        vec![Action::Show],
        "done offers no state change"
    );
}

#[test]
fn command_args_map_each_action_to_its_plaesy_tasks_command() {
    assert_eq!(
        command_args(Action::Start, "t1", ""),
        ["tasks", "start", "t1"]
    );
    assert_eq!(
        command_args(Action::Complete, "t1", ""),
        ["tasks", "complete", "t1"]
    );
    assert_eq!(
        command_args(Action::Block, "t1", "menunggu API"),
        ["tasks", "block", "t1", "menunggu API"]
    );
    assert_eq!(
        command_args(Action::Show, "t1", ""),
        ["tasks", "show", "t1"]
    );
}

#[test]
fn title_is_the_first_heading_or_the_id() {
    assert_eq!(title_from("intro\n# Judul tugas\nisi", "x"), "Judul tugas");
    assert_eq!(title_from("tanpa heading", "fallback-id"), "fallback-id");
    assert_eq!(title_from("#   \n", "fallback-id"), "fallback-id");
}

#[test]
fn scan_reads_every_status_folder_in_lifecycle_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_task(dir.path(), "done", "z-selesai", "# Selesai\n");
    write_task(dir.path(), "backlog", "b-nanti", "# Nanti\n");
    write_task(dir.path(), "doing", "d-kerjakan", "# Kerjakan\n");
    write_task(dir.path(), "todo", "a-siap", "tanpa judul\n");

    let items = scan(dir.path());
    let got: Vec<(&str, &str)> = items
        .iter()
        .map(|i| (i.status.as_str(), i.id.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("backlog", "b-nanti"),
            ("todo", "a-siap"),
            ("doing", "d-kerjakan"),
            ("done", "z-selesai"),
        ]
    );
    assert_eq!(items[1].title, "a-siap", "no heading falls back to the id");
}

#[test]
fn scan_of_a_repo_without_tasks_is_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(scan(dir.path()).is_empty());
    assert_eq!(render(Lang::En, &[]), "no tasks");
}

#[test]
fn render_lists_status_id_and_title() {
    let items = vec![TaskItem {
        id: "t1".to_owned(),
        status: "todo".to_owned(),
        title: "Judul".to_owned(),
    }];
    assert_eq!(render(Lang::En, &items), "[todo] t1 Judul");
}
