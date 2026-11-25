//! Roles for `plaesy chat`: a named profile with a system prompt, an optional default provider,
//! skills loaded into the system prompt, and an optional tool list that can only narrow what the
//! `/mcp` commands may call. Roles come from `roles` in `settings.json`: user roles first, then
//! repository roles, and a repository role with the same name replaces the user's.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

pub const DEFAULT_ROLE: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Role {
    pub name: String,
    pub system: Option<String>,
    pub provider: Option<String>,
    pub skills: Vec<String>,
    pub tools: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawRole {
    #[serde(default)]
    system: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    tools: Vec<String>,
}

/// Roles from a settings value (`"roles": {"name": {...}}`).
pub fn parse_roles(value: &Value) -> Result<Vec<Role>, String> {
    let Some(map) = value.get("roles") else {
        return Ok(Vec::new());
    };
    let raw: BTreeMap<String, RawRole> =
        serde_json::from_value(map.clone()).map_err(|err| format!("roles: {err}"))?;
    Ok(raw
        .into_iter()
        .map(|(name, role)| Role {
            name,
            system: role.system,
            provider: role.provider,
            skills: role.skills,
            tools: role.tools,
        })
        .collect())
}

/// User roles, then repository roles. A repository role replaces a user role with the same name.
pub fn merge(user: Vec<Role>, repo: Vec<Role>) -> Vec<Role> {
    let mut out: Vec<Role> = user
        .into_iter()
        .filter(|role| !repo.iter().any(|r| r.name == role.name))
        .collect();
    out.extend(repo);
    out
}

/// The named role. `default` exists even when no settings define it: no prompt, no provider.
pub fn find(roles: &[Role], name: &str) -> Option<Role> {
    roles
        .iter()
        .find(|role| role.name == name)
        .cloned()
        .or_else(|| {
            (name == DEFAULT_ROLE).then(|| Role {
                name: DEFAULT_ROLE.to_owned(),
                ..Role::default()
            })
        })
}

/// A skill name must be one folder name: no separators and no `.` or `..`.
fn valid_skill_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', '.'])
}

/// The system text for a role: its prompt, then each skill's `SKILL.md` from the store. A skill
/// that is missing or has a bad name is a warning, not an error.
pub fn system_text(role: &Role, skills_dir: &Path) -> (Option<String>, Vec<String>) {
    let mut parts = Vec::new();
    let mut warnings = Vec::new();
    if let Some(system) = &role.system {
        parts.push(system.clone());
    }
    for skill in &role.skills {
        if !valid_skill_name(skill) {
            warnings.push(format!("skill name {skill:?} is not valid"));
            continue;
        }
        match std::fs::read_to_string(skills_dir.join(skill).join("SKILL.md")) {
            Ok(text) => parts.push(format!("# Skill: {skill}\n\n{}", text.trim())),
            Err(_) => warnings.push(format!(
                "skill {skill} not found in {}",
                skills_dir.display()
            )),
        }
    }
    if parts.is_empty() {
        (None, warnings)
    } else {
        (Some(parts.join("\n\n")), warnings)
    }
}

/// An empty tool list allows every tool the server allows; a non-empty one narrows it.
pub fn tool_allowed(role: &Role, tool: &str) -> bool {
    role.tools.is_empty() || role.tools.iter().any(|name| name == tool)
}

#[cfg(test)]
#[path = "chat_roles_test.rs"]
mod chat_roles_test;

/// Reads one skill for `/skill`: its `SKILL.md`, under a heading that names it.
///
/// # Errors
///
/// A message when the name is not a valid skill name or the skill has no `SKILL.md`.
pub fn load_skill(skills_dir: &Path, name: &str) -> Result<String, String> {
    if !valid_skill_name(name) {
        return Err(format!("nama skill tidak valid: {name}"));
    }
    std::fs::read_to_string(skills_dir.join(name).join("SKILL.md"))
        .map(|text| format!("# Skill: {name}\n\n{}", text.trim()))
        .map_err(|_| format!("skill {name} tidak ditemukan di {}", skills_dir.display()))
}
