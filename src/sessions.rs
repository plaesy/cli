//! Project-local, self-reported session status registry.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub agent: String,
    pub state: String,
    pub task: Option<String>,
    pub files: Vec<String>,
    pub updated_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid session data: {0}")]
    Invalid(String),
    #[error("session not found: {0}")]
    NotFound(String),
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
fn validate(id: &str, agent: &str, state: &str, ttl: u64) -> Result<(), Error> {
    if id.trim().is_empty() || agent.trim().is_empty() {
        return Err(Error::Invalid(
            "session id and agent label must be non-empty".into(),
        ));
    }
    if state.trim().is_empty() || state.len() > 80 || state.chars().any(char::is_control) {
        return Err(Error::Invalid(
            "state must be 1..=80 printable characters".into(),
        ));
    }
    if ttl == 0 || ttl > 86_400 {
        return Err(Error::Invalid("TTL must be 1..=86400 seconds".into()));
    }
    Ok(())
}
fn locked<T>(root: &Path, f: impl FnOnce(&Path) -> Result<T, Error>) -> Result<T, Error> {
    let dir = root.join(".plaesy");
    fs::create_dir_all(&dir)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".sessions.lock"))?;
    lock.lock()?;
    let result = f(&dir);
    let _ = lock.unlock();
    result
}
fn read(dir: &Path) -> Result<Vec<Session>, Error> {
    let p = dir.join("sessions.json");
    if !p.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&fs::read(p)?)?)
}
fn write(dir: &Path, list: &[Session]) -> Result<(), Error> {
    let tmp = dir.join(format!(".sessions-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, serde_json::to_vec_pretty(list)?)?;
    fs::rename(tmp, dir.join("sessions.json"))?;
    Ok(())
}
/// Upsert a self-reported live session state with a TTL.
pub fn report(
    root: &Path,
    id: &str,
    agent: &str,
    state: &str,
    task: Option<&str>,
    files: &[String],
    ttl: u64,
) -> Result<Session, Error> {
    validate(id, agent, state, ttl)?;
    if files
        .iter()
        .any(|f| f.starts_with('/') || f.split('/').any(|x| x == ".." || x.is_empty()))
    {
        return Err(Error::Invalid(
            "files must be project-relative paths without traversal".into(),
        ));
    }
    locked(root, |dir| {
        let mut sessions: Vec<_> = read(dir)?
            .into_iter()
            .filter(|s| s.expires_at > now() && s.id != id)
            .collect();
        let item = Session {
            id: id.to_owned(),
            agent: agent.to_owned(),
            state: state.to_owned(),
            task: task.map(str::to_owned),
            files: files.to_vec(),
            updated_at: now(),
            expires_at: now() + ttl,
        };
        sessions.push(item.clone());
        write(dir, &sessions)?;
        Ok(item)
    })
}
/// Refresh the liveness TTL without changing the reported state.
pub fn heartbeat(root: &Path, id: &str, ttl: u64) -> Result<Session, Error> {
    validate(id, "heartbeat", "active", ttl)?;
    locked(root, |dir| {
        let mut sessions = read(dir)?;
        let item = sessions
            .iter_mut()
            .find(|s| s.id == id && s.expires_at > now())
            .ok_or_else(|| Error::NotFound(id.into()))?;
        item.updated_at = now();
        item.expires_at = now() + ttl;
        let result = item.clone();
        write(dir, &sessions)?;
        Ok(result)
    })
}
/// List sessions and identify expired records as stale rather than active.
pub fn list(root: &Path, include_stale: bool) -> Result<Vec<(Session, bool)>, Error> {
    locked(root, |dir| {
        let mut out: Vec<_> = read(dir)?
            .into_iter()
            .map(|s| {
                let stale = s.expires_at <= now();
                (s, stale)
            })
            .filter(|(_, stale)| include_stale || !stale)
            .collect();
        out.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        Ok(out)
    })
}
/// Stop a session's record explicitly.
pub fn stop(root: &Path, id: &str) -> Result<(), Error> {
    locked(root, |dir| {
        let mut sessions = read(dir)?;
        let n = sessions.len();
        sessions.retain(|s| s.id != id);
        if n == sessions.len() {
            return Err(Error::NotFound(id.into()));
        }
        write(dir, &sessions)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_list_heartbeat_stop() {
        let root = tempfile::tempdir().unwrap();
        let files = vec!["src/main.rs".into()];
        let s = report(
            root.path(),
            "s-1",
            "agent",
            "working",
            Some("T-1"),
            &files,
            600,
        )
        .unwrap();
        assert_eq!(list(root.path(), false).unwrap()[0].0, s);
        assert!(heartbeat(root.path(), "s-1", 600).unwrap().expires_at >= s.expires_at);
        stop(root.path(), "s-1").unwrap();
        assert!(list(root.path(), true).unwrap().is_empty());
    }
    #[test]
    fn expired_session_is_stale_and_hidden_by_default() {
        let root = tempfile::tempdir().unwrap();
        let files = vec![];
        report(root.path(), "s", "a", "working", None, &files, 1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(list(root.path(), false).unwrap().is_empty());
        assert_eq!(list(root.path(), true).unwrap().len(), 1);
        assert!(list(root.path(), true).unwrap()[0].1);
    }
    #[test]
    fn rejects_traversing_file_names() {
        let root = tempfile::tempdir().unwrap();
        assert!(report(
            root.path(),
            "s",
            "a",
            "working",
            None,
            &["../outside".into()],
            60
        )
        .is_err());
    }
}
