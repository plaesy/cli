//! Plaesy-managed native typed decision models.

use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use laya::agent::LoadOptions;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MODEL_ID: &str = "laya";
const REPO: &str = "convaiinnovations/laya";
pub const REVISION: &str = "06d956120490bbaebaf4bc0e23377a10be65171f";
pub const FILES: &[(&str, u64, &str)] = &[
    (
        "model.safetensors",
        842_609_210,
        "891102d372688fc2a094dac56a384bc537b87c63f21f9f3dac0be2b7cbc8d86c",
    ),
    (
        "encoder/config.json",
        2_083,
        "bf3ab80598fdccf414855a2ce80f22859e4492d06ca8a62ddd1cfb63972f8979",
    ),
    (
        "rl_agent_config.json",
        745,
        "ae287b56bbcf5f8c4f4541ae9dfd00c914c4c48b940b8398c3058af37ba92bbd",
    ),
    (
        "tokenizer/tokenizer.json",
        3_583_228,
        "6c8aaa9a542084f2457eab775d4eeb51f92a70c0fd9de28d5edb0ddec3c08d30",
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DecisionsConfig {
    pub model: String,
}

impl Default for DecisionsConfig {
    fn default() -> Self {
        Self {
            model: MODEL_ID.into(),
        }
    }
}

/// Serializes cooperating Plaesy callers while model files are in use.
struct ModelLock(fs::File);

fn ensure_private_model_directories(
    target: &Path,
) -> Result<crate::memory::fs_safety::DirHandle, String> {
    let cache = dirs::cache_dir().ok_or("cannot determine platform cache directory")?;
    ensure_private_model_directories_at(&cache, target)
}

fn ensure_private_model_directories_at(
    cache: &Path,
    target: &Path,
) -> Result<crate::memory::fs_safety::DirHandle, String> {
    use std::path::Component;

    let relative = target
        .strip_prefix(cache)
        .map_err(|_| "model path is outside the platform cache directory")?;
    if !cache.is_absolute() || !target.is_absolute() {
        return Err("model cache paths must be absolute".to_owned());
    }
    let mut current = crate::memory::fs_safety::DirHandle::open_nofollow(Path::new("/"))
        .map_err(|e| format!("opening filesystem root safely: {e}"))?;
    for component in cache.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::RootDir | Component::Prefix(_)) {
                continue;
            }
            return Err("model cache path is not normalized".to_owned());
        };
        current = current.open_dir_beneath(Path::new(name)).map_err(|e| {
            format!(
                "opening model cache ancestor {} safely: {e}",
                name.to_string_lossy()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = current.metadata().map_err(|e| e.to_string())?;
            let uid = unsafe { libc::geteuid() };
            // Sticky bit (S_ISVTX) as a fixed u32 mask: libc declares it as u16 on macOS and u32
            // on Linux, so the libc constant does not type-check against `mode()` on both.
            const STICKY_BIT: u32 = 0o1000;
            let root_sticky = metadata.uid() == 0 && metadata.mode() & STICKY_BIT != 0;
            if metadata.uid() != 0 && metadata.uid() != uid {
                return Err(format!(
                    "cache ancestor {} is not owned by root or the current user",
                    name.to_string_lossy()
                ));
            }
            if metadata.mode() & 0o022 != 0 && !root_sticky {
                return Err(format!(
                    "cache ancestor {} is writable by group or others",
                    name.to_string_lossy()
                ));
            }
        }
    }
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("model path must contain only normal components below cache".to_owned());
        };
        let child = Path::new(name);
        let next = match current.open_dir_beneath(child) {
            Ok(handle) => handle,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match current.create_dir(child, 0o700) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(format!("creating model directory safely: {error}")),
                }
                current.open_dir_beneath(child).map_err(|e| {
                    format!("opening model directory {} safely: {e}", child.display())
                })?
            }
            Err(error) => return Err(format!("opening model directory safely: {error}")),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = next.metadata().map_err(|e| e.to_string())?;
            if metadata.uid() != unsafe { libc::geteuid() } {
                return Err(format!(
                    "model directory {} is not owned by the current user",
                    child.display()
                ));
            }
            if metadata.mode() & 0o777 != 0o700 {
                next.set_permissions(0o700)
                    .map_err(|e| format!("restricting model directory {}: {e}", child.display()))?;
            }
        }
        current = next;
    }
    Ok(current)
}

fn lock_model() -> Result<ModelLock, String> {
    let target = model_path()?;
    let cache_parent = target.parent().ok_or("model path has no parent")?;
    let handle = ensure_private_model_directories(cache_parent)?;
    let file = handle
        .open_or_create_file(Path::new(".decisions.lock"), 0o600)
        .map_err(|e| format!("opening model lock safely: {e}"))?;
    file.lock()
        .map_err(|e| format!("locking model {}: {e}", target.display()))?;
    Ok(ModelLock(file))
}

impl Drop for ModelLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

struct RemoveFileWithinOnDrop {
    directory: crate::memory::fs_safety::DirHandle,
    name: PathBuf,
}

impl Drop for RemoveFileWithinOnDrop {
    fn drop(&mut self) {
        let _ = self.directory.remove_file(&self.name);
    }
}

struct RemoveDirectoryOnDrop(Option<(crate::memory::fs_safety::DirHandle, PathBuf)>);

impl Drop for RemoveDirectoryOnDrop {
    fn drop(&mut self) {
        if let Some((parent, name)) = self.0.take() {
            let _ = remove_model_tree(&parent, &name);
        }
    }
}

fn remove_model_tree(
    parent: &crate::memory::fs_safety::DirHandle,
    name: &Path,
) -> Result<(), String> {
    let model = parent
        .open_dir_beneath(name)
        .map_err(|e| format!("opening model directory for removal: {e}"))?;
    for (relative, _, _) in FILES {
        let path = Path::new(relative);
        let mut parts = path.components();
        let first = parts.next().ok_or("empty model artifact path")?;
        let std::path::Component::Normal(first) = first else {
            return Err(format!("invalid model artifact path {relative}"));
        };
        let (directory, leaf) = if let Some(std::path::Component::Normal(second)) = parts.next() {
            match model.open_dir_beneath(Path::new(first)) {
                Ok(directory) => (directory, second),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(format!("opening artifact directory {first:?}: {error}")),
            }
        } else {
            (model.try_clone().map_err(|e| e.to_string())?, first)
        };
        match directory.remove_file(Path::new(leaf)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("removing model artifact {relative}: {error}")),
        }
    }
    for dir in ["encoder", "tokenizer"] {
        match model.remove_dir(Path::new(dir)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("removing model artifact directory {dir}: {error}")),
        }
    }
    parent
        .remove_dir(name)
        .map_err(|e| format!("removing model directory: {e}"))
}

fn create_temp_directory(
    parent: &crate::memory::fs_safety::DirHandle,
    label: &str,
) -> Result<PathBuf, String> {
    for _ in 0..128 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!(".{label}.{}.{}.part", std::process::id(), sequence);
        match parent.create_dir(Path::new(&name), 0o700) {
            Ok(()) => return Ok(PathBuf::from(name)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("creating staging directory {name}: {error}")),
        }
    }
    Err("could not allocate a unique staging directory in the model cache".to_owned())
}

fn config_path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|p| p.join("plaesy").join("decisions.json"))
        .ok_or_else(|| "cannot determine platform config directory".into())
}

fn open_or_create_directory_path(
    path: &Path,
) -> Result<crate::memory::fs_safety::DirHandle, String> {
    use std::path::Component;

    if !path.is_absolute() {
        return Err(format!(
            "directory path {} must be absolute",
            path.display()
        ));
    }
    let mut handle = crate::memory::fs_safety::DirHandle::open_nofollow(Path::new("/"))
        .map_err(|e| format!("opening filesystem root safely: {e}"))?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::RootDir) {
                continue;
            }
            return Err(format!(
                "directory path {} is not normalized",
                path.display()
            ));
        };
        let name = Path::new(name);
        match handle.open_dir_beneath(name) {
            Ok(child) => handle = child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match handle.create_dir(name, 0o700) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(format!(
                            "creating directory {} safely: {error}",
                            name.display()
                        ))
                    }
                }
                handle = handle.open_dir_beneath(name).map_err(|e| {
                    format!("opening created directory {} safely: {e}", name.display())
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "opening directory {} safely: {error}",
                    name.display()
                ))
            }
        }
    }
    Ok(handle)
}

// Only the Linux symlink and atomic-publish tests call this helper.
#[cfg(all(test, target_os = "linux"))]
fn save_config_at(config: &DecisionsConfig, path: &Path) -> Result<(), String> {
    save_config_with_path(config, path)
}

fn save_config_with_path(config: &DecisionsConfig, path: &Path) -> Result<(), String> {
    if config.model != MODEL_ID {
        return Err(format!(
            "unsupported decision model {:?}; available model: {MODEL_ID}",
            config.model
        ));
    }
    let parent = path.parent().ok_or("config path has no parent")?;
    let handle = open_or_create_directory_path(parent)?;
    let name = path.file_name().ok_or("config path has no file name")?;
    let mut temp = None;
    let mut file = None;
    for _ in 0..128 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let candidate = PathBuf::from(format!(
            ".decisions.json.{}.{}.part",
            std::process::id(),
            sequence
        ));
        match handle.create_new_file(&candidate) {
            Ok(created) => {
                temp = Some(candidate);
                file = Some(created);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("creating config staging file: {error}")),
        }
    }
    let temp = temp.ok_or("could not allocate a unique config staging file")?;
    let mut file = file.ok_or("config staging file was not created")?;
    let _cleanup = RemoveFileWithinOnDrop {
        directory: handle
            .try_clone()
            .map_err(|e| format!("cloning config directory handle: {e}"))?,
        name: temp.clone(),
    };
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("writing config staging file: {e}"))?;
        drop(file);
        handle
            .replace_within(&temp, Path::new(name))
            .map_err(|e| format!("replacing {}: {e}", path.display()))?;
        handle
            .sync()
            .map_err(|e| format!("syncing config directory {}: {e}", parent.display()))
    })();
    if result.is_err() {
        let _ = handle.remove_file(&temp);
    }
    result
}

pub fn save_config(config: &DecisionsConfig) -> Result<(), String> {
    save_config_with_path(config, &config_path()?)
}

fn model_path() -> Result<PathBuf, String> {
    crate::memory::paths::models_root()
        .map(|p| p.join(MODEL_ID).join(REVISION))
        .ok_or_else(|| "cannot determine platform model cache directory".into())
}

pub fn load_config() -> Result<DecisionsConfig, String> {
    let path = config_path()?;
    let parent = path.parent().ok_or("config path has no parent")?;
    let handle = match crate::memory::fs_safety::DirHandle::open_nofollow(parent) {
        Ok(handle) => handle,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DecisionsConfig::default());
        }
        Err(error) => {
            return Err(format!(
                "opening config directory {} safely: {error}",
                parent.display()
            ));
        }
    };
    let name = path.file_name().ok_or("config path has no file name")?;
    let mut file = match handle.open_file_read_relative_nofollow(Path::new(name)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DecisionsConfig::default());
        }
        Err(error) => return Err(format!("opening config {} safely: {error}", path.display())),
    };
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|e| format!("reading {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))
}

static TEMP_FILE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn sha256_file(mut file: fs::File, path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    // Heap buffer: a stack array of this size overflows the Windows main thread (1 MiB).
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_model_at(
    handle: &crate::memory::fs_safety::DirHandle,
    display_dir: &Path,
) -> Result<(), String> {
    for (name, size, hash) in FILES {
        let path = display_dir.join(name);
        let file = handle
            .open_file_read_relative_nofollow(Path::new(name))
            .map_err(|e| format!("opening model artifact {} safely: {e}", path.display()))?;
        let metadata = file
            .metadata()
            .map_err(|e| format!("checking model artifact {}: {e}", path.display()))?;
        if metadata.len() != *size {
            return Err(format!(
                "model file {} has size {}, expected {size}",
                path.display(),
                metadata.len()
            ));
        }
        let actual = sha256_file(file, &path)?;
        if actual != *hash {
            return Err(format!(
                "model file {} failed SHA-256 verification",
                path.display()
            ));
        }
    }
    Ok(())
}

pub fn model_status() -> Result<bool, String> {
    let _lock = lock_model()?;
    let path = model_path()?;
    let parent = path.parent().ok_or("model path has no parent")?;
    let parent_handle = crate::memory::fs_safety::DirHandle::open_nofollow(parent)
        .map_err(|e| format!("opening model cache safely: {e}"))?;
    let name = path.file_name().ok_or("model path has no name")?;
    match parent_handle.open_dir_beneath(Path::new(name)) {
        Ok(model) => {
            validate_model_at(&model, &path)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("opening model directory safely: {error}")),
    }
}

pub fn pull() -> Result<(), String> {
    let _lock = lock_model()?;
    let target = model_path()?;
    let parent = target.parent().ok_or("model path has no parent")?;
    let parent_handle = ensure_private_model_directories(parent)?;
    if let Some(name) = target.file_name() {
        match parent_handle.open_dir_beneath(Path::new(name)) {
            Ok(installed) => {
                validate_model_at(&installed, &target)?;
                println!("Laya {REVISION} is installed at {}", target.display());
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("opening installed model safely: {error}")),
        }
    } else {
        return Err("model path has no name".to_owned());
    }
    let staging_name = create_temp_directory(&parent_handle, REVISION)?;
    let staging_handle = parent_handle
        .open_dir_beneath(&staging_name)
        .map_err(|e| format!("opening model staging directory safely: {e}"))?;
    let _staging_cleanup = RemoveDirectoryOnDrop(Some((
        parent_handle
            .try_clone()
            .map_err(|e| format!("cloning model cache handle: {e}"))?,
        staging_name.clone(),
    )));
    let result = (|| {
        for (name, expected_size, expected_hash) in FILES {
            let url = format!("https://huggingface.co/{REPO}/resolve/{REVISION}/{name}");
            let relative = Path::new(name);
            let mut components = relative.components().peekable();
            let mut destination_dir = staging_handle
                .try_clone()
                .map_err(|e| format!("cloning staging model directory handle: {e}"))?;
            while let Some(component) = components.next() {
                let std::path::Component::Normal(component) = component else {
                    return Err(format!("invalid model manifest path {name}"));
                };
                if components.peek().is_some() {
                    let component_path = Path::new(component);
                    if destination_dir.create_dir(component_path, 0o700).is_err() {
                        // It may have been created for another artifact in the same manifest.
                        if destination_dir.open_dir_beneath(component_path).is_err() {
                            return Err(format!("creating model artifact directory for {name}"));
                        }
                    }
                    destination_dir = destination_dir
                        .open_dir_beneath(component_path)
                        .map_err(|e| format!("opening model artifact directory for {name}: {e}"))?;
                } else {
                    println!("Downloading {name} ({expected_size} bytes)");
                    let agent: ureq::Agent = ureq::Agent::config_builder()
                        .timeout_global(Some(Duration::from_secs(60 * 30)))
                        .build()
                        .into();
                    let mut response = agent
                        .get(&url)
                        .call()
                        .map_err(|e| format!("downloading {url}: {e}"))?;
                    let sequence =
                        TEMP_FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let part_name = PathBuf::from(format!(
                        ".download.{}.{}.part",
                        std::process::id(),
                        sequence
                    ));
                    let mut file = destination_dir
                        .create_new_file(&part_name)
                        .map_err(|e| format!("creating staging file for {name}: {e}"))?;
                    let _part_cleanup = RemoveFileWithinOnDrop {
                        directory: destination_dir
                            .try_clone()
                            .map_err(|e| format!("cloning staging directory handle: {e}"))?,
                        name: part_name.clone(),
                    };
                    let mut limited = response
                        .body_mut()
                        .as_reader()
                        .take(expected_size.saturating_add(1));
                    std::io::copy(&mut limited, &mut file)
                        .map_err(|e| format!("downloading {url}: {e}"))?;
                    file.sync_all()
                        .map_err(|e| format!("syncing staging file for {name}: {e}"))?;
                    if file.metadata().map_err(|e| e.to_string())?.len() != *expected_size {
                        return Err(format!("downloaded {name} has unexpected size"));
                    }
                    file.rewind()
                        .map_err(|e| format!("rewinding staging file for {name}: {e}"))?;
                    if sha256_file(file, Path::new(name))? != *expected_hash {
                        return Err(format!("downloaded {name} failed SHA-256 verification"));
                    }
                    destination_dir
                        .replace_within(&part_name, Path::new(component))
                        .map_err(|e| format!("promoting staging file for {name}: {e}"))?;
                }
            }
        }
        validate_model_at(&staging_handle, &parent.join(&staging_name))?;
        parent_handle
            .replace_within(
                &staging_name,
                Path::new(target.file_name().ok_or("model path has no name")?),
            )
            .map_err(|e| format!("promoting model to {}: {e}", target.display()))?;
        parent_handle
            .sync()
            .map_err(|e| format!("syncing model cache {}: {e}", parent.display()))?;
        Ok(())
    })();
    result?;
    println!("Installed Laya {REVISION} at {}", target.display());
    Ok(())
}

pub fn remove() -> Result<(), String> {
    let _lock = lock_model()?;
    let path = model_path()?;
    let name = path.file_name().ok_or("model path has no name")?;
    let parent = path.parent().ok_or("model path has no parent")?;
    let parent_handle = match crate::memory::fs_safety::DirHandle::open_nofollow(parent) {
        Ok(handle) => handle,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("Removed Laya model files from {}", path.display());
            return Ok(());
        }
        Err(error) => return Err(format!("opening model cache safely: {error}")),
    };
    match parent_handle.open_dir_beneath(Path::new(name)) {
        Ok(_) => remove_model_tree(&parent_handle, Path::new(name))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("opening model directory safely: {error}")),
    }
    parent_handle
        .sync()
        .map_err(|e| format!("syncing model cache {}: {e}", parent.display()))?;
    println!("Removed Laya model files from {}", path.display());
    Ok(())
}

pub fn choose(state: &str, prompt: &str, options: &[String]) -> Result<serde_json::Value, String> {
    if options.len() < 2 {
        return Err("provide at least two --option values".into());
    }
    if options.iter().any(|option| option.trim().is_empty()) {
        return Err("decision options must not be empty".into());
    }
    let unique: std::collections::BTreeSet<&str> = options.iter().map(String::as_str).collect();
    if unique.len() != options.len() {
        return Err("decision options must be unique".into());
    }
    if prompt.trim().is_empty() || state.trim().is_empty() {
        return Err("state and prompt must not be empty".into());
    }
    let cfg = load_config()?;
    if cfg.model != MODEL_ID {
        return Err(format!("unsupported configured model: {}", cfg.model));
    }
    let _lock = lock_model()?;
    let dir = model_path()?;
    let parent = dir.parent().ok_or("model path has no parent")?;
    let parent_handle = crate::memory::fs_safety::DirHandle::open_nofollow(parent)
        .map_err(|e| format!("opening model cache safely: {e}"))?;
    let model_name = dir.file_name().ok_or("model path has no name")?;
    let handle = parent_handle
        .open_dir_beneath(Path::new(model_name))
        .map_err(|e| format!("run `plaesy decisions model pull` first: {e}"))?;
    let mut artifacts = Vec::with_capacity(FILES.len());
    for (name, expected_size, expected_hash) in FILES {
        let path = dir.join(name);
        let file = handle
            .open_file_read_relative_nofollow(Path::new(name))
            .map_err(|e| {
                format!(
                    "reading verified model artifact {} safely: {e}",
                    path.display()
                )
            })?;
        let mut bytes = Vec::with_capacity((*expected_size).min(16 * 1024 * 1024) as usize);
        file.take(expected_size.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| format!("reading verified model artifact {}: {e}", path.display()))?;
        if bytes.len() as u64 != *expected_size {
            return Err(format!(
                "model artifact {} has unexpected size",
                path.display()
            ));
        }
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));
        if actual_hash != *expected_hash {
            return Err(format!(
                "model artifact {} changed after validation",
                path.display()
            ));
        }
        artifacts.push(bytes);
    }
    let agent = laya::Agent::load_from_bytes(
        &artifacts[2],
        &artifacts[1],
        &artifacts[0],
        &artifacts[3],
        LoadOptions::default(),
    )
    .map_err(|e| format!("loading decision model: {e}"))?;
    let state: laya::State =
        serde_json::from_str(state).unwrap_or_else(|_| serde_json::Value::String(state.to_owned()));
    let mut question = serde_json::Map::new();
    question.insert("type".into(), serde_json::Value::String("choice".into()));
    question.insert(
        "instructions".into(),
        serde_json::Value::String(prompt.into()),
    );
    let mut criteria = serde_json::Map::new();
    for option in options {
        criteria.insert(option.clone(), serde_json::Value::String(option.clone()));
    }
    question.insert("criteria".into(), serde_json::Value::Object(criteria));
    let mut questions = laya::Questions::new();
    questions.insert("recommendation".into(), serde_json::Value::Object(question));
    let result = agent
        .system_one(&state, &questions)
        .map_err(|e| format!("Laya inference failed: {e}"))?;
    let output = result.to_json();
    let answer = output
        .pointer("/answers/recommendation")
        .ok_or("Laya result missing answer")?;
    let choice = answer
        .get("choice")
        .and_then(serde_json::Value::as_str)
        .ok_or("Laya result missing choice")?;
    if !options.iter().any(|item| item == choice) {
        return Err(format!(
            "Laya returned a choice outside the supplied options: {choice}"
        ));
    }
    Ok(output)
}

#[cfg(test)]
#[path = "decisions_test.rs"]
mod decisions_test;
