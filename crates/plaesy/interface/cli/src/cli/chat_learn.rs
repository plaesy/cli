//! `/learn`: the provider reads the session and proposes what to keep, as in Hermes. The proposal
//! is shown first. `/learn save` then records each lesson as a decision and writes the skill, if
//! there is one, to the skills folder. Nothing is written before `save`.

use std::fs;
use std::path::Path;

use serde_json::Value;

use super::chat_roles::valid_skill_name;
use crate::delegate::Message;

/// A skill the session suggests: a name, a one-line description, and the body of `SKILL.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDraft {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// What `/learn` proposes from one session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Proposal {
    pub lessons: Vec<String>,
    pub skill: Option<SkillDraft>,
}

/// Messages for the proposal call: the instructions, the session so far, and the ask.
#[must_use]
pub fn messages(history: Vec<Message>) -> Vec<Message> {
    let mut out = vec![Message::system(
        "Dari percakapan ini, keluarkan HANYA satu objek JSON tanpa teks lain: \
         {\"lessons\":[\"pelajaran singkat\", ...], \"skill\":null atau \
         {\"name\":\"nama-kebab\",\"description\":\"satu kalimat\",\"body\":\"isi markdown \
         langkah-langkah\"}}. Isi skill hanya jika ada prosedur yang bisa dipakai ulang. \
         Pelajaran harus berupa fakta atau keputusan dari percakapan, bukan rangkuman."
            .to_owned(),
    )];
    out.extend(history);
    out.push(Message::user("Keluarkan JSON-nya sekarang.".to_owned()));
    out
}

/// Reads the proposal out of a reply. The reply may wrap the object in other text; the first `{`
/// to the last `}` is parsed.
///
/// # Errors
///
/// A message when no JSON object can be found or it does not have the expected fields.
pub fn parse(reply: &str) -> Result<Proposal, String> {
    let start = reply.find('{').ok_or("balasan tidak berisi JSON")?;
    let end = reply.rfind('}').ok_or("balasan tidak berisi JSON")?;
    if end < start {
        return Err("balasan tidak berisi JSON".to_owned());
    }
    let value: Value = serde_json::from_str(&reply[start..=end]).map_err(|e| e.to_string())?;
    let lessons = value
        .get("lessons")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let skill = match value.get("skill") {
        Some(obj) if obj.is_object() => {
            let field = |key: &str| {
                obj.get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            };
            Some(SkillDraft {
                name: field("name"),
                description: field("description"),
                body: field("body"),
            })
        }
        _ => None,
    };
    Ok(Proposal { lessons, skill })
}

/// Writes the skill to `skills_dir/<name>/SKILL.md`, replacing an existing one. Returns true when
/// a skill of that name already existed.
///
/// # Errors
///
/// A message when the name is not a valid skill name or the file cannot be written.
pub fn write_skill(skills_dir: &Path, draft: &SkillDraft) -> Result<bool, String> {
    if !valid_skill_name(&draft.name) {
        return Err(format!("nama skill tidak valid: {}", draft.name));
    }
    if draft.body.is_empty() {
        return Err("isi skill kosong".to_owned());
    }
    let folder = skills_dir.join(&draft.name);
    let path = folder.join("SKILL.md");
    let existed = path.exists();
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let text = format!(
        "---\nname: {}\ndescription: {}\n---\n\n{}\n",
        draft.name, draft.description, draft.body
    );
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(existed)
}

#[cfg(test)]
#[path = "chat_learn_test.rs"]
mod chat_learn_test;
