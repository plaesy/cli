//! Unit tests for `tests` split out of `decisions.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn model_manifest_has_valid_hashes_and_relative_paths() {
    for (name, size, hash) in FILES {
        assert!(!Path::new(name).is_absolute());
        assert!(!name.split('/').any(|part| part == ".."));
        assert!(*size > 0);
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|c| c.is_ascii_hexdigit()));
    }
    assert_eq!(REVISION.len(), 40);
}

#[test]
fn config_defaults_to_supported_model_and_rejects_unknown() {
    assert_eq!(DecisionsConfig::default().model, MODEL_ID);
    assert!(save_config(&DecisionsConfig {
        model: "unsupported".into()
    })
    .is_err());
}

#[cfg(unix)]
#[test]
fn model_directories_are_created_private_and_existing_modes_restricted() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let temp = tempfile::tempdir_in("/tmp").unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let cache = temp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o755)).unwrap();
    let target = cache.join("models/laya/revision/model.safetensors");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::set_permissions(target.parent().unwrap(), fs::Permissions::from_mode(0o775)).unwrap();

    ensure_private_model_directories_at(&cache, &target).unwrap();
    assert_eq!(
        fs::metadata(&cache).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(target.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    let linked_root = cache.join("linked");
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, &linked_root).unwrap();
    let linked_target = linked_root.join("revision");
    assert!(ensure_private_model_directories_at(&cache, &linked_target).is_err());
    drop(temp);
}

#[cfg(unix)]
#[test]
fn model_path_rejects_symlink_and_dangling_symlink() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let actual_dir = temp.path().join("actual");
    fs::create_dir(&actual_dir).unwrap();
    let symlink_dir = temp.path().join("linked-dir");
    symlink(&actual_dir, &symlink_dir).unwrap();
    assert!(crate::memory::fs_safety::DirHandle::open_nofollow(&symlink_dir).is_err());

    let dangling = temp.path().join("dangling");
    symlink(temp.path().join("missing"), &dangling).unwrap();
    assert!(crate::memory::fs_safety::DirHandle::open_nofollow(&dangling).is_err());

    let absent = temp.path().join("absent");
    assert!(matches!(
        crate::memory::fs_safety::DirHandle::open_nofollow(&absent),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    ));
}

#[cfg(unix)]
#[test]
fn validation_rejects_symlinked_artifact_directory() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let model = temp.path().join("model");
    let outside = temp.path().join("outside");
    fs::create_dir(&model).unwrap();
    fs::create_dir(&outside).unwrap();
    symlink(&outside, model.join("encoder")).unwrap();
    let handle = crate::memory::fs_safety::DirHandle::open_nofollow(&model).unwrap();
    assert!(validate_model_at(&handle, &model).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn save_config_rejects_symlinked_directory_without_writing_outside() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("config");
    fs::create_dir(&config_root).unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, config_root.join("plaesy")).unwrap();

    let path = config_root.join("plaesy/decisions.json");
    assert!(save_config_at(&DecisionsConfig::default(), &path).is_err());
    assert!(!outside.join("decisions.json").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn save_config_creates_directories_and_publishes_atomically() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config/plaesy/decisions.json");

    save_config_at(&DecisionsConfig::default(), &path).unwrap();
    let saved: DecisionsConfig = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(saved, DecisionsConfig::default());
}

#[test]
fn sha256_file_does_not_overflow_a_small_stack() {
    // Regression: the 1 MiB read buffer lived on the stack and overflowed the
    // Windows main thread (1 MiB reserve), aborting `model pull` after download.
    let path = std::env::temp_dir().join(format!("plaesy-sha256-stack-{}.bin", std::process::id()));
    let payload = vec![0xA5_u8; 2 * 1024 * 1024];
    fs::write(&path, &payload).expect("write temp payload");
    let expected = format!("{:x}", Sha256::digest(&payload));
    let file_path = path.clone();
    let actual = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let file = fs::File::open(&file_path).expect("open temp payload");
            sha256_file(file, &file_path)
        })
        .expect("spawn small-stack thread")
        .join()
        .expect("hashing thread must not abort")
        .expect("hash temp payload");
    let _ = fs::remove_file(&path);
    assert_eq!(actual, expected);
}
