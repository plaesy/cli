//! Unit tests for `tests` split out of `fs_safety.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn rejects_symlinked_directory() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(DirHandle::open_nofollow(&link).is_err());
    assert!(DirHandle::open_nofollow(&real).is_ok());
}

#[test]
fn rejects_symlinked_ancestor_directory() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let child = real.join("child");
    std::fs::create_dir(&child).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(DirHandle::open_nofollow(&link.join("child")).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn relative_file_open_rejects_symlinked_intermediate_and_leaf() {
    use std::io::Read;
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let actual = dir.path().join("actual");
    std::fs::create_dir(&actual).unwrap();
    std::fs::write(actual.join("value"), b"inside").unwrap();
    let linked_dir = dir.path().join("linked");
    symlink(&actual, &linked_dir).unwrap();
    let linked_file = dir.path().join("linked-file");
    symlink(actual.join("value"), &linked_file).unwrap();
    let handle = DirHandle::open_nofollow(dir.path()).unwrap();

    assert!(handle
        .open_file_read_relative_nofollow(Path::new("linked/value"))
        .is_err());
    assert!(handle
        .open_file_read_relative_nofollow(Path::new("linked-file"))
        .is_err());
    let mut file = handle
        .open_file_read_relative_nofollow(Path::new("actual/value"))
        .unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "inside");
}

#[test]
fn relative_file_read_stays_on_pinned_directory_after_path_replacement() {
    use std::io::Read;

    let root = tempfile::tempdir().unwrap();
    let topic = root.path().join("spec");
    std::fs::create_dir(&topic).unwrap();
    std::fs::write(topic.join("spec.md"), b"inside spec").unwrap();
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let root_handle = DirHandle::open_nofollow(root.path()).unwrap();
    let topic_handle = root_handle.open_dir_beneath(Path::new("spec")).unwrap();

    std::fs::rename(&topic, root.path().join("spec-original")).unwrap();
    std::os::unix::fs::symlink(&outside, &topic).unwrap();
    let mut file = topic_handle
        .open_file_read_nofollow(Path::new("spec.md"))
        .unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    assert_eq!(contents, "inside spec");
    assert!(!outside.join("spec.md").exists());
}

#[test]
fn open_dir_beneath_pins_directory_against_path_replacement() {
    let root = tempfile::tempdir().unwrap();
    let tasks = root.path().join("tasks");
    let task = tasks.join("todo/item");
    std::fs::create_dir_all(&task).unwrap();
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let root_handle = DirHandle::open_nofollow(&tasks).unwrap();
    let task_handle = root_handle
        .open_dir_beneath(Path::new("todo/item"))
        .unwrap();

    std::fs::rename(&task, tasks.join("todo/item-original")).unwrap();
    std::os::unix::fs::symlink(&outside, &task).unwrap();
    let mut file = task_handle.create_new_file(Path::new("sentinel")).unwrap();
    use std::io::Write;
    file.write_all(b"pinned").unwrap();

    assert_eq!(
        std::fs::read(outside.join("sentinel")).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(
        std::fs::read(tasks.join("todo/item-original/sentinel")).unwrap(),
        b"pinned"
    );
    assert!(root_handle
        .open_dir_beneath(Path::new("todo/item/../item-original"))
        .is_err());
}

#[test]
fn create_link_remove_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let handle = DirHandle::open_nofollow(dir.path()).unwrap();
    let tmp = Path::new(".note.tmp");
    let dest = Path::new("note.md");
    {
        use std::io::Write;
        let mut f = handle.create_new_file(tmp).unwrap();
        f.write_all(b"hello").unwrap();
        f.sync_all().unwrap();
    }
    handle.link_within(tmp, dest).unwrap();
    handle.remove_file(tmp).unwrap();
    handle.sync().unwrap();
    let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
    assert_eq!(content, "hello");
    // Destination already exists: must fail rather than silently replace it.
    assert!(handle.create_new_file(dest).is_err());
}

#[test]
fn descriptor_relative_mutations_reject_path_escape() {
    use std::io::Write;

    let root = tempfile::tempdir().unwrap();
    let held = root.path().join("held");
    let outside = root.path().join("outside");
    std::fs::create_dir(&held).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let sentinel = outside.join("sentinel");
    std::fs::write(&sentinel, b"unchanged").unwrap();
    let handle = DirHandle::open_nofollow(&held).unwrap();

    assert!(handle
        .create_new_file(Path::new("../outside/created"))
        .is_err());
    assert!(handle.create_new_file(&sentinel).is_err());
    assert!(handle
        .link_within(Path::new("../outside/sentinel"), Path::new("linked"))
        .is_err());
    assert!(handle
        .link_within(Path::new("sentinel"), Path::new("../outside/linked"))
        .is_err());
    assert!(handle
        .replace_within(Path::new("../outside/sentinel"), Path::new("replaced"))
        .is_err());
    assert!(handle
        .replace_within(Path::new("missing"), Path::new("../outside/replaced"))
        .is_err());
    assert!(handle
        .remove_file(Path::new("../outside/sentinel"))
        .is_err());
    assert!(handle.remove_file(&sentinel).is_err());

    let mut created = handle.create_new_file(Path::new("inside")).unwrap();
    created.write_all(b"inside").unwrap();
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");
    assert!(!outside.join("created").exists());
    assert!(!outside.join("linked").exists());
    assert!(!outside.join("replaced").exists());
}

#[test]
fn open_or_create_file_does_not_block_on_fifo() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe");
    let fifo_c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    let rc = unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) };
    assert_eq!(rc, 0, "mkfifo failed: {}", io::Error::last_os_error());

    let handle = DirHandle::open_nofollow(dir.path()).unwrap();
    assert!(handle
        .open_or_create_file(Path::new("pipe"), 0o600)
        .is_err());
}

#[test]
fn create_new_file_rejects_existing_symlink_target() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let bait = outside.path().join("bait");
    std::fs::write(&bait, "outside").unwrap();
    let link_name = Path::new("escape");
    std::os::unix::fs::symlink(&bait, dir.path().join(link_name)).unwrap();
    let handle = DirHandle::open_nofollow(dir.path()).unwrap();
    // O_EXCL already rejects anything pre-existing at that name, symlink or not.
    assert!(handle.create_new_file(link_name).is_err());
    assert_eq!(std::fs::read_to_string(&bait).unwrap(), "outside");
}
