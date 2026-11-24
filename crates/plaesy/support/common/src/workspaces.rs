//! Named workspace groups over the per-user project registry.
//!
//! A group keeps a list of project roots (stable ID and path). Groups live in `workspaces.json`
//! next to `projects.json` and share its lock. Membership is never pruned silently: `members`
//! reports a root that no longer exists as unavailable, and only an explicit `remove` drops it.
use crate::projects::{self, Project};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const FILE: &str = "workspaces.json";
const MAX_NAME_LEN: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub name: String,
    pub members: Vec<Project>,
}

/// A group member together with whether its root is still a Plaesy project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub project: Project,
    pub available: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("workspace name must be 1-64 letters, digits, '-' or '_': {0}")]
    InvalidName(String),
    #[error("workspace not found: {0}")]
    NotFound(String),
    #[error("workspace already exists: {0}")]
    AlreadyExists(String),
    #[error("project {selector} is not in workspace {workspace}")]
    NotMember { workspace: String, selector: String },
    #[error(transparent)]
    Project(#[from] projects::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Creates an empty workspace group.
///
/// # Errors
///
/// The name is invalid or already taken, or the store cannot be read or written.
pub fn create(name: &str) -> Result<(), Error> {
    update(|groups| create_in(groups, name))
}

/// Deletes a workspace group. The projects it named stay registered.
///
/// # Errors
///
/// The group does not exist, or the store cannot be read or written.
pub fn delete(name: &str) -> Result<(), Error> {
    update(|groups| delete_in(groups, name))
}

/// Registers `path` as a project, then adds it to the group. Returns the project and whether it
/// was newly added (`false` when it was already a member).
///
/// # Errors
///
/// `path` is not a Plaesy project root, the group does not exist, or the store fails.
pub fn add(name: &str, path: &Path) -> Result<(Project, bool), Error> {
    // Register before taking the store lock: registration takes the same lock.
    let project = projects::register(path)?;
    let added = update(|groups| add_in(groups, name, project.clone()))?;
    Ok((project, added))
}

/// Removes a member matched by stable ID or path.
///
/// # Errors
///
/// The group does not exist, the selector is not a member, or the store fails.
pub fn remove(name: &str, selector: &str) -> Result<Project, Error> {
    update(|groups| remove_in(groups, name, selector))
}

/// Lists every group, sorted by name.
///
/// # Errors
///
/// The store exists but cannot be read.
pub fn list() -> Result<Vec<Workspace>, Error> {
    load(&store_file()?)
}

/// Lists the members of one group, marking those whose root is gone.
///
/// # Errors
///
/// The group does not exist, or the store cannot be read.
pub fn members(name: &str) -> Result<Vec<Member>, Error> {
    members_in(&load(&store_file()?)?, name)
}

fn store_file() -> Result<PathBuf, Error> {
    Ok(projects::data_dir()?.join(FILE))
}

/// Runs a read-modify-write of the store under the shared registry lock.
fn update<T>(f: impl FnOnce(&mut Vec<Workspace>) -> Result<T, Error>) -> Result<T, Error> {
    let dir = projects::data_dir()?;
    let file = dir.join(FILE);
    projects::with_lock(&dir, || {
        let mut groups = load(&file)?;
        let out = f(&mut groups)?;
        save(&file, &groups)?;
        Ok(out)
    })
}

fn load(file: &Path) -> Result<Vec<Workspace>, Error> {
    if !file.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&fs::read(file)?)?)
}

fn save(file: &Path, groups: &[Workspace]) -> Result<(), Error> {
    let parent = file.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "workspace file has no parent directory",
        )
    })?;
    let tmp = parent.join(format!(".workspaces-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, serde_json::to_vec_pretty(groups)?)?;
    fs::rename(tmp, file)?;
    Ok(())
}

fn validate_name(name: &str) -> Result<(), Error> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidName(name.to_owned()))
    }
}

fn group_mut<'a>(groups: &'a mut [Workspace], name: &str) -> Result<&'a mut Workspace, Error> {
    groups
        .iter_mut()
        .find(|group| group.name == name)
        .ok_or_else(|| Error::NotFound(name.to_owned()))
}

fn create_in(groups: &mut Vec<Workspace>, name: &str) -> Result<(), Error> {
    validate_name(name)?;
    if groups.iter().any(|group| group.name == name) {
        return Err(Error::AlreadyExists(name.to_owned()));
    }
    groups.push(Workspace {
        name: name.to_owned(),
        members: Vec::new(),
    });
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(())
}

fn delete_in(groups: &mut Vec<Workspace>, name: &str) -> Result<(), Error> {
    let before = groups.len();
    groups.retain(|group| group.name != name);
    if groups.len() == before {
        return Err(Error::NotFound(name.to_owned()));
    }
    Ok(())
}

fn add_in(groups: &mut [Workspace], name: &str, project: Project) -> Result<bool, Error> {
    let group = group_mut(groups, name)?;
    if group
        .members
        .iter()
        .any(|m| m.id == project.id || m.path == project.path)
    {
        return Ok(false);
    }
    group.members.push(project);
    Ok(true)
}

fn remove_in(groups: &mut [Workspace], name: &str, selector: &str) -> Result<Project, Error> {
    let group = group_mut(groups, name)?;
    let position = group
        .members
        .iter()
        .position(|m| m.id == selector || m.path == Path::new(selector))
        .ok_or_else(|| Error::NotMember {
            workspace: name.to_owned(),
            selector: selector.to_owned(),
        })?;
    Ok(group.members.remove(position))
}

fn members_in(groups: &[Workspace], name: &str) -> Result<Vec<Member>, Error> {
    let group = groups
        .iter()
        .find(|group| group.name == name)
        .ok_or_else(|| Error::NotFound(name.to_owned()))?;
    Ok(group
        .members
        .iter()
        .map(|project| Member {
            available: projects::is_project_root(&project.path),
            project: project.clone(),
        })
        .collect())
}

#[cfg(test)]
#[path = "workspaces_test.rs"]
mod workspaces_test;
