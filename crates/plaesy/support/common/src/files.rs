//! Local advisory file-ownership claims for coordinating Plaesy sessions.
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Component, Path};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claim {
    pub path: String,
    pub agent: String,
    pub task: Option<String>,
    pub lease_id: String,
    pub expires_at: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid project-relative path: {0}")]
    InvalidPath(String),
    #[error("file is claimed by {agent} until {expires_at} (lease {lease_id})")]
    Conflict {
        agent: String,
        expires_at: u64,
        lease_id: String,
    },
    #[error("no matching active claim for {agent} on {path}")]
    NotOwner { agent: String, path: String },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn normalize(root: &Path, raw: &str) -> Result<String, Error> {
    let path = Path::new(raw);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Error::InvalidPath(raw.to_owned()));
    }
    let canonical_root = fs::canonicalize(root)?;
    let mut ancestor = root.to_path_buf();
    for component in path.components() {
        ancestor.push(component.as_os_str());
        if !ancestor.exists() {
            break;
        }
        let canonical = fs::canonicalize(&ancestor)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(Error::InvalidPath(raw.to_owned()));
        }
    }
    if root.join(path).exists() {
        let canonical = fs::canonicalize(root.join(path))?;
        if !canonical.starts_with(&canonical_root) {
            return Err(Error::InvalidPath(raw.to_owned()));
        }
    }
    Ok(path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn with_lock<T>(root: &Path, f: impl FnOnce(&Path) -> Result<T, Error>) -> Result<T, Error> {
    let dir = root.join(".plaesy/presence");
    fs::create_dir_all(&dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".lock"))?;
    lock.lock()?;
    let result = f(&dir);
    let _ = lock.unlock();
    result
}

fn load(dir: &Path) -> Result<Vec<Claim>, Error> {
    let path = dir.join("claims.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
/// List active advisory claims, optionally filtered to one path.
pub fn list(root: &Path, raw_path: Option<&str>) -> Result<Vec<Claim>, Error> {
    let path = raw_path.map(|p| normalize(root, p)).transpose()?;
    with_lock(root, |dir| {
        let claims: Vec<_> = load(dir)?
            .into_iter()
            .filter(|c| c.expires_at > now() && path.as_ref().is_none_or(|p| p == &c.path))
            .collect();
        Ok(claims)
    })
}

/// Read active claims in stable path order for best-effort polling clients.
pub fn snapshot(root: &Path) -> Result<Vec<Claim>, Error> {
    with_lock(root, |dir| {
        let mut claims: Vec<_> = load(dir)?
            .into_iter()
            .filter(|c| c.expires_at > now())
            .collect();
        claims.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(claims)
    })
}

/// Changes observed between two snapshots. This is not a durable event log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "event", content = "claim", rename_all = "snake_case")]
pub enum ClaimChange {
    Claimed(Claim),
    Released(Claim),
}

pub fn diff_snapshots(previous: &[Claim], current: &[Claim]) -> Vec<ClaimChange> {
    let mut changes = Vec::new();
    for claim in previous {
        if !current.iter().any(|next| next.lease_id == claim.lease_id) {
            changes.push(ClaimChange::Released(claim.clone()));
        }
    }
    for claim in current {
        if !previous.iter().any(|old| old.lease_id == claim.lease_id) {
            changes.push(ClaimChange::Claimed(claim.clone()));
        }
    }
    changes
}

fn save(dir: &Path, claims: &[Claim]) -> Result<(), Error> {
    let tmp = dir.join(format!(".claims-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, serde_json::to_vec_pretty(claims)?)?;
    fs::rename(tmp, dir.join("claims.json"))?;
    Ok(())
}

/// Create an advisory expiring claim; does not prevent filesystem writes.
pub fn claim(
    root: &Path,
    raw_path: &str,
    agent: &str,
    task: Option<&str>,
    ttl_seconds: u64,
) -> Result<Claim, Error> {
    if agent.trim().is_empty() {
        return Err(Error::InvalidPath("agent label is empty".into()));
    }
    if ttl_seconds == 0 || ttl_seconds > 86_400 {
        return Err(Error::InvalidPath("TTL must be 1..=86400 seconds".into()));
    }
    let path = normalize(root, raw_path)?;
    with_lock(root, |dir| {
        let mut claims: Vec<_> = load(dir)?
            .into_iter()
            .filter(|c| c.expires_at > now())
            .collect();
        if let Some(existing) = claims.iter().find(|c| c.path == path && c.agent != agent) {
            return Err(Error::Conflict {
                agent: existing.agent.clone(),
                expires_at: existing.expires_at,
                lease_id: existing.lease_id.clone(),
            });
        }
        claims.retain(|c| !(c.path == path && c.agent == agent));
        let item = Claim {
            path,
            agent: agent.to_owned(),
            task: task.map(str::to_owned),
            lease_id: uuid::Uuid::new_v4().to_string(),
            expires_at: now() + ttl_seconds,
        };
        claims.push(item.clone());
        save(dir, &claims)?;
        Ok(item)
    })
}

/// Release only the caller's own advisory claim.
pub fn release(root: &Path, raw_path: &str, agent: &str) -> Result<(), Error> {
    release_matching(root, raw_path, agent, None)
}

/// Release a specific lease so a stale session cannot remove its replacement.
pub fn release_lease(
    root: &Path,
    raw_path: &str,
    agent: &str,
    lease_id: &str,
) -> Result<(), Error> {
    release_matching(root, raw_path, agent, Some(lease_id))
}

fn release_matching(
    root: &Path,
    raw_path: &str,
    agent: &str,
    lease_id: Option<&str>,
) -> Result<(), Error> {
    let path = normalize(root, raw_path)?;
    with_lock(root, |dir| {
        let mut claims = load(dir)?;
        let before = claims.len();
        claims.retain(|c| {
            !(c.path == path
                && c.agent == agent
                && lease_id.is_none_or(|lease_id| c.lease_id == lease_id))
        });
        if before == claims.len() {
            return Err(Error::NotOwner {
                agent: agent.to_owned(),
                path,
            });
        }
        save(dir, &claims)
    })
}

#[cfg(test)]
#[path = "files_test.rs"]
mod files_test;
