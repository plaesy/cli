//! `template` helpers split out of `mod.rs`.

use super::*;

/// Optional values used to fill managed template front matter.
pub(crate) struct TemplateOverrides<'a> {
    pub folder: Option<&'a str>,
    pub values: &'a [String],
    pub id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub project: Option<&'a str>,
    pub scope: &'a [String],
    pub supersedes: &'a [String],
}

/// Render a managed note from a vault-local template and return its folder, Markdown and graph metadata.
pub(crate) fn render_template(
    root: &Path,
    template: &str,
    title: &str,
    body: &str,
    overrides: TemplateOverrides<'_>,
) -> Result<(String, String, Option<GraphFields>)> {
    let TemplateOverrides {
        folder,
        values,
        id,
        status,
        project,
        scope,
        supersedes,
    } = overrides;
    if template.is_empty()
        || template.contains('\\')
        || template.split('/').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    {
        return Err(MemoryError::ConfigError(
            "template path must use lowercase kebab-case components (without .md)".into(),
        ));
    }
    let template_path = root.join("templates").join(template).with_extension("md");
    let real_root = std::fs::canonicalize(root).map_err(|source| MemoryError::IoError {
        path: root.display().to_string(),
        source,
    })?;
    let real_template =
        std::fs::canonicalize(&template_path).map_err(|source| MemoryError::IoError {
            path: template_path.display().to_string(),
            source,
        })?;
    if !real_template.starts_with(&real_root) || !real_template.is_file() {
        return Err(MemoryError::ConfigError(format!(
            "template path resolves outside the vault or is not a file: {}",
            template_path.display()
        )));
    }
    let size = std::fs::metadata(&real_template)
        .map_err(|source| MemoryError::IoError {
            path: real_template.display().to_string(),
            source,
        })?
        .len();
    if size > MAX_BODY_BYTES as u64 {
        return Err(MemoryError::ConfigError(format!(
            "template is larger than {MAX_BODY_BYTES} bytes: {}",
            real_template.display()
        )));
    }
    let raw = std::fs::read_to_string(&real_template).map_err(|source| MemoryError::IoError {
        path: real_template.display().to_string(),
        source,
    })?;
    let normalized = raw.replace("\r\n", "\n");
    let (frontmatter, markdown) = split_frontmatter(&normalized)?;
    let mut metadata: serde_yaml::Mapping = serde_yaml::from_str(frontmatter).map_err(|e| {
        MemoryError::ConfigError(format!(
            "invalid YAML front matter in {}: {e}",
            template_path.display()
        ))
    })?;
    let note_type = yaml_string(&metadata, "type").ok_or_else(|| {
        MemoryError::ConfigError(format!(
            "template {template} is missing a string 'type' field"
        ))
    })?;
    if id.is_some_and(|value| !is_uuid(value)) {
        return Err(MemoryError::ConfigError(
            "--id must be a canonical UUID when --template is used".into(),
        ));
    }
    let note_id = id.map(str::to_owned).unwrap_or_else(new_uuid);
    let today = Utc::now().date_naive().to_string();
    set_yaml(&mut metadata, "schema", serde_yaml::Value::Number(1.into()));
    set_yaml(
        &mut metadata,
        "id",
        serde_yaml::Value::String(note_id.clone()),
    );
    set_yaml(
        &mut metadata,
        "title",
        serde_yaml::Value::String(title.to_owned()),
    );
    set_yaml(
        &mut metadata,
        "created",
        serde_yaml::Value::String(today.clone()),
    );
    set_yaml(&mut metadata, "updated", serde_yaml::Value::String(today));
    set_yaml(
        &mut metadata,
        "source",
        serde_yaml::Value::String("tool:plaesy".into()),
    );
    set_yaml_if_missing(
        &mut metadata,
        "status",
        serde_yaml::Value::String("draft".into()),
    );
    set_yaml_if_missing(
        &mut metadata,
        "scope",
        serde_yaml::Value::Sequence(Vec::new()),
    );
    set_yaml_if_missing(
        &mut metadata,
        "tags",
        serde_yaml::Value::Sequence(Vec::new()),
    );
    set_yaml_if_missing(
        &mut metadata,
        "trust",
        serde_yaml::Value::String("owner".into()),
    );
    set_yaml_if_missing(
        &mut metadata,
        "classification",
        serde_yaml::Value::String("internal".into()),
    );
    set_yaml_if_missing(
        &mut metadata,
        "share_with",
        serde_yaml::Value::Sequence(Vec::new()),
    );
    if let Some(status) = status {
        set_yaml(
            &mut metadata,
            "status",
            serde_yaml::Value::String(status.to_owned()),
        );
    }
    if let Some(project) = project {
        set_yaml(
            &mut metadata,
            "project",
            serde_yaml::Value::String(project.to_owned()),
        );
    }
    if !scope.is_empty() {
        set_yaml(
            &mut metadata,
            "scope",
            serde_yaml::Value::Sequence(
                scope
                    .iter()
                    .cloned()
                    .map(serde_yaml::Value::String)
                    .collect(),
            ),
        );
    }
    if !supersedes.is_empty() {
        set_yaml(
            &mut metadata,
            "supersedes",
            serde_yaml::Value::Sequence(
                supersedes
                    .iter()
                    .cloned()
                    .map(serde_yaml::Value::String)
                    .collect(),
            ),
        );
    }
    let mut markdown = markdown.to_owned();
    let mut replacements = std::collections::HashMap::new();
    for assignment in values {
        let (key, value) = assignment.split_once('=').ok_or_else(|| {
            MemoryError::ConfigError(format!("template value must use KEY=VALUE: {assignment}"))
        })?;
        let key = key.trim();
        if key.is_empty() {
            return Err(MemoryError::ConfigError(
                "template value key must not be empty".into(),
            ));
        }
        let marker = format!("{{{{{key}}}}}");
        if !markdown.contains(&marker) {
            return Err(MemoryError::ConfigError(format!(
                "template '{template}' has no placeholder named '{key}'"
            )));
        }
        if replacements
            .insert(key.to_owned(), value.to_owned())
            .is_some()
        {
            return Err(MemoryError::ConfigError(format!(
                "template placeholder '{key}' was provided more than once"
            )));
        }
    }
    let body_key = match note_type.as_str() {
        "capture" => Some("content"),
        "knowledge" => Some("ONE_SENTENCE_SUMMARY"),
        "decision" => Some("CHOICE_MADE"),
        "project" => Some("CLEAR_OUTCOME_DESCRIPTION"),
        "preference" => Some("CLEAR_PREFERENCE_STATEMENT"),
        "company" => Some("ONE_PARAGRAPH_SUMMARY"),
        "task" => Some("TASK_DESCRIPTION"),
        _ => None,
    };
    if let Some(key) = body_key {
        // Consume the body placeholder even when the caller omits a body. Leaving
        // `{{...}}` markers in a newly created note makes it look like a broken
        // template instead of an intentionally blank section.
        replacements
            .entry(key.to_owned())
            .or_insert_with(|| body.to_owned());
    }
    let title_key = match note_type.as_str() {
        "knowledge" | "decision" => "TITLE",
        "project" => "PROJECT_NAME",
        "company" => "COMPANY_NAME",
        "task" => "TASK_TITLE",
        "preference" => "PREFERENCE_NAME",
        _ => "TITLE",
    };
    replacements
        .entry(title_key.into())
        .or_insert_with(|| title.into());
    for (key, value) in replacements {
        markdown = markdown.replace(&format!("{{{{{key}}}}}"), &value);
    }
    if !body.trim().is_empty() && (body_key.is_none() || !markdown.contains(body)) {
        markdown.push_str("\n\n## Additional details\n\n");
        markdown.push_str(body.trim());
        markdown.push('\n');
    }
    let graph_status = yaml_string(&metadata, "status");
    let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(metadata)).map_err(|e| {
        MemoryError::ConfigError(format!("could not serialize template metadata: {e}"))
    })?;
    let content = format!("---\n{yaml}---\n\n{}", markdown.trim_start());
    let folder = match folder {
        Some(folder) => folder.to_owned(),
        None => match note_type.as_str() {
            "capture" => "inbox/fleeting".into(),
            "decision" => "decisions".into(),
            "idea" => "ideas".into(),
            "company" => "companies".into(),
            "project" => format!("projects/{}", slugify(title)),
            _ => {
                return Err(MemoryError::ConfigError(format!(
                    "template '{template}' needs an explicit --folder (type '{note_type}')"
                )));
            }
        },
    };
    let graph = if crate::memory::graph::NOTE_TYPES.contains(&note_type.as_str()) {
        Some(GraphFields {
            note_type,
            id: note_id,
            status: status.map(str::to_owned).or(graph_status),
            project: project.map(str::to_owned),
            scope: scope.to_vec(),
            supersedes: supersedes.to_vec(),
        })
    } else {
        None
    };
    Ok((folder, content, graph))
}

fn split_frontmatter(text: &str) -> Result<(&str, &str)> {
    let rest = text.strip_prefix("---\n").ok_or_else(|| {
        MemoryError::ConfigError("template must start with YAML front matter ('---')".into())
    })?;
    let end = rest.find("\n---\n").ok_or_else(|| {
        MemoryError::ConfigError("template YAML front matter is missing a closing '---'".into())
    })?;
    Ok((&rest[..end], &rest[end + 5..]))
}

pub(super) fn yaml_key(key: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(key.to_owned())
}

pub(super) fn yaml_string(mapping: &serde_yaml::Mapping, key: &str) -> Option<String> {
    mapping
        .get(yaml_key(key))
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_owned)
}

fn set_yaml(mapping: &mut serde_yaml::Mapping, key: &str, value: serde_yaml::Value) {
    mapping.insert(yaml_key(key), value);
}

fn set_yaml_if_missing(mapping: &mut serde_yaml::Mapping, key: &str, value: serde_yaml::Value) {
    mapping.entry(yaml_key(key)).or_insert(value);
}
