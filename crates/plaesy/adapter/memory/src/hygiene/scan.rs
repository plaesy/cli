//! `scan` helpers split out of `mod.rs`.

use super::*;

impl SearchEngine {
    /// Wikilinks of `path`: its outgoing links, or (with `incoming`) the links pointing at it.
    ///
    /// `path` may be a full indexed path, a path without extension, or a bare file stem.
    ///
    /// # Errors
    ///
    /// - `DatabaseError` if the link table cannot be read
    pub async fn links(&self, path: &str, incoming: bool) -> Result<Vec<LinkRef>> {
        let db = self.database();
        let paths: Vec<String> = db
            .document_paths()
            .await?
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        let wanted = crate::memory::storage::resolve_link(path, &paths)
            .unwrap_or_else(|| path.replace('\\', "/"));
        Ok(db
            .all_links()
            .await?
            .into_iter()
            .filter(|l| {
                if incoming {
                    l.resolved_path.as_deref() == Some(wanted.as_str())
                } else {
                    l.source_path == wanted
                }
            })
            .collect())
    }

    /// Run the hygiene checks over every indexed document.
    ///
    /// # Errors
    ///
    /// - `DatabaseError` if documents or links cannot be read
    pub async fn hygiene(&self, opts: &HygieneOptions) -> Result<HygieneReport> {
        let db = self.database();
        let now = opts.now.unwrap_or_else(Utc::now);
        let stale_days = opts
            .stale_after_days
            .unwrap_or(self.search_config().stale_after_days);
        let mut docs = db.all_documents().await?;
        let scope = opts.scope.as_ref().map(|scope| {
            scope
                .to_string_lossy()
                .replace('\\', "/")
                .trim_matches('/')
                .to_owned()
        });
        if let Some(scope) = scope.as_deref() {
            if scope.is_empty() || scope.split('/').any(|part| part == ".." || part == ".") {
                return Err(crate::memory::error::MemoryError::ConfigError(
                    "hygiene scope must be a normalized vault-relative path".into(),
                ));
            }
            let prefix = format!("{scope}/");
            docs.retain(|doc| doc.path == scope || doc.path.starts_with(&prefix));
        }
        let mut findings: Vec<Finding> = Vec::new();

        for doc in &docs {
            let (path, md, body) = (&doc.path, &doc.metadata, &doc.body);
            let meta = build_meta(path, md, now, stale_days);
            let tier = meta.tier.as_deref();

            if let Some(raw) = md.get("tier").and_then(|v| v.as_str()) {
                if normalize_tier(raw).is_none() {
                    findings.push(finding(
                        "bad_tier",
                        path,
                        None,
                        format!("tier '{raw}' is not one of core, episodic, semantic, procedural, archive"),
                        "FACT",
                    ));
                }
            }

            if opts.scan_secrets {
                for (kind, line) in secret_references(body) {
                    findings.push(finding(
                        "possible_secret",
                        path,
                        Some(line + doc.body_start_line - 1),
                        format!("possible {kind}; value omitted"),
                        "INFERRED",
                    ));
                }
            }

            // Verification metadata is only demanded of notes that make claims about the world.
            if matches!(tier, Some("core") | Some("semantic") | Some("procedural")) {
                let has_source = md
                    .get("source")
                    .and_then(|value| value.as_str())
                    .is_some_and(|source| !source.trim().is_empty());
                if !has_source {
                    findings.push(finding(
                        "missing_source",
                        path,
                        None,
                        format!("{} note has no non-empty `source`", tier.unwrap_or("?")),
                        "FACT",
                    ));
                }
                if meta.last_verified.is_none() {
                    findings.push(finding(
                        "missing_last_verified",
                        path,
                        None,
                        format!("{} note has no `last_verified`", tier.unwrap_or("?")),
                        "FACT",
                    ));
                }
                if meta.trust.is_none() {
                    findings.push(finding(
                        "missing_confidence",
                        path,
                        None,
                        format!("{} note has no `confidence`", tier.unwrap_or("?")),
                        "FACT",
                    ));
                }
            }

            for key in ["valid_from", "valid_to"] {
                if let Some(raw) = md.get(key) {
                    let Some(value) = raw.as_str() else {
                        findings.push(finding(
                            "invalid_validity_date",
                            path,
                            None,
                            format!("`{key}` must be a YYYY-MM-DD or RFC 3339 string"),
                            "FACT",
                        ));
                        continue;
                    };
                    if parse_date(value).is_none() {
                        findings.push(finding(
                            "invalid_validity_date",
                            path,
                            None,
                            format!("`{key}` value is not a valid date or RFC 3339 timestamp"),
                            "FACT",
                        ));
                    }
                }
            }
            if let (Some(from), Some(to)) = (
                md.get("valid_from")
                    .and_then(|value| value.as_str())
                    .and_then(parse_date),
                md.get("valid_to")
                    .and_then(|value| value.as_str())
                    .and_then(parse_date),
            ) {
                if from > to {
                    findings.push(finding(
                        "inverted_validity_range",
                        path,
                        None,
                        "`valid_from` is later than `valid_to`".to_string(),
                        "FACT",
                    ));
                }
            }

            if meta.stale {
                let lv = meta.last_verified.as_deref().unwrap_or("?");
                let days = age_days(lv, now).unwrap_or(0);
                findings.push(finding(
                    "stale",
                    path,
                    None,
                    format!("last verified {lv} ({days} days ago; window is {stale_days})"),
                    "FACT",
                ));
            }

            if meta.status.as_deref() == Some("superseded") && !md.contains_key("superseded_by") {
                findings.push(finding(
                    "superseded_without_replacement",
                    path,
                    None,
                    "status is `superseded` but `superseded_by` is not set".to_string(),
                    "FACT",
                ));
            }

            // Templates reference paths that exist only once instantiated.
            let is_template = path.ends_with(".template.md")
                || path.replace('\\', "/").split('/').any(|s| s == "templates");
            if let (Some(root), false) = (&opts.root, is_template) {
                let mut bases = vec![root.clone()];
                if let Some(parent) = root.parent() {
                    bases.push(parent.to_path_buf());
                }
                let note_path = root.join(path);
                let mut project_root = root.clone();
                let project_relative = note_path.strip_prefix(root.join("projects")).ok();
                let mapped_roots = project_relative
                    .and_then(|relative| relative.components().next())
                    .and_then(|project| match project {
                        std::path::Component::Normal(name) => {
                            opts.project_roots.get(&name.to_string_lossy().to_string())
                        }
                        _ => None,
                    });
                if let Some(roots) = mapped_roots {
                    bases.extend(roots.iter().cloned());
                }
                if let Some(std::path::Component::Normal(project)) =
                    project_relative.and_then(|relative| relative.components().next())
                {
                    let candidate = root.join("projects").join(project);
                    if candidate.is_dir() {
                        project_root = candidate;
                    }
                }
                let mut ancestor = note_path.parent();
                while let Some(directory) = ancestor {
                    if !directory.starts_with(&project_root) {
                        break;
                    }
                    bases.push(directory.to_path_buf());
                    if directory == project_root {
                        break;
                    }
                    ancestor = directory.parent();
                }
                for (token, line) in path_references(body) {
                    let exists = bases.iter().any(|base| base.join(&token).exists())
                        || mapped_roots.is_some_and(|roots| {
                            roots.iter().any(|root| exists_in_source_root(root, &token))
                        });
                    if !exists {
                        findings.push(finding(
                            "dead_path_reference",
                            path,
                            Some(line + doc.body_start_line - 1),
                            format!(
                                "`{token}` does not exist in the note's folder hierarchy, project, vault, or mapped source repository"
                            ),
                            "INFERRED",
                        ));
                    }
                }
            }
        }

        if let Some(root) = &opts.root {
            findings.extend(markdown_links::check(root, &docs));
        }

        for l in db.all_links().await? {
            if l.resolved_path.is_none() {
                findings.push(finding(
                    "broken_link",
                    &l.source_path,
                    Some(l.line),
                    format!("[[{}]] matches no indexed document", l.target),
                    "FACT",
                ));
            }
        }

        // Project/decision/context graph: broken relations and conflicting supersession.
        // Read-only and additive — a vault with no `type:`-tagged notes reports nothing here.
        let graph = crate::memory::graph::load_graph(&db).await?;
        for m in &graph.malformed {
            findings.push(finding(
                "malformed_graph_note",
                &m.path,
                None,
                format!("type: {} — {}", m.note_type, m.reason),
                "FACT",
            ));
        }
        let project_ids: std::collections::HashSet<&str> =
            graph.project_ids().into_iter().collect();
        for n in &graph.notes {
            if n.note_type == "project" {
                continue;
            }
            if let Some(p) = &n.project {
                if !project_ids.contains(p.as_str()) {
                    findings.push(finding(
                        "broken_project_reference",
                        &n.path,
                        None,
                        format!("project '{p}' matches no `type: project` note"),
                        "FACT",
                    ));
                }
            }
            for s in &n.scope {
                if !project_ids.contains(s.as_str()) {
                    findings.push(finding(
                        "broken_scope_reference",
                        &n.path,
                        None,
                        format!("scope '{s}' matches no `type: project` note"),
                        "FACT",
                    ));
                }
            }
        }
        let (conflicts, _) = crate::memory::graph::supersession_map(&graph);
        for c in &conflicts {
            findings.push(finding(
                "conflicting_supersession",
                "(graph)",
                None,
                format!(
                    "decision '{}' is claimed superseded by multiple active decisions: {}",
                    c.target_id,
                    c.claimants.join(", ")
                ),
                "FACT",
            ));
        }

        // Duplicate `id`: two notes (any type) declaring the same id mean lookups such as
        // `resolve_project` silently pick whichever one they see first — every other claimant
        // is invisible to callers with no error raised anywhere in the write path.
        let mut by_id: std::collections::HashMap<&str, Vec<&crate::memory::graph::GraphNote>> =
            std::collections::HashMap::new();
        for n in &graph.notes {
            by_id.entry(n.id.as_str()).or_default().push(n);
        }
        for (id, notes) in &by_id {
            if notes.len() > 1 {
                let paths: Vec<&str> = notes.iter().map(|n| n.path.as_str()).collect();
                findings.push(finding(
                    "duplicate_graph_id",
                    paths[0],
                    None,
                    format!(
                        "id '{id}' is declared by {} notes: {}",
                        notes.len(),
                        paths.join(", ")
                    ),
                    "FACT",
                ));
            }
        }

        // A decision that lists its own id in `supersedes` is a self-referencing cycle: it can
        // never be a legitimate replacement for itself, and it would make `supersession_map`
        // report the decision as superseding itself with no warning otherwise.
        let decision_ids: std::collections::HashSet<&str> = graph
            .of_type("decision")
            .into_iter()
            .map(|decision| decision.id.as_str())
            .collect();
        for n in graph.of_type("decision") {
            if n.supersedes.iter().any(|s| s == &n.id) {
                findings.push(finding(
                    "self_superseding_decision",
                    &n.path,
                    None,
                    format!("decision '{}' lists itself in `supersedes`", n.id),
                    "FACT",
                ));
            }
            for target in &n.supersedes {
                if !decision_ids.contains(target.as_str()) {
                    findings.push(finding(
                        "dangling_supersedes_reference",
                        &n.path,
                        None,
                        format!("`supersedes` references missing decision '{target}'"),
                        "FACT",
                    ));
                }
            }

            let metadata = docs
                .iter()
                .find(|doc| doc.path == n.path)
                .map(|doc| &doc.metadata);
            let replacement = metadata
                .and_then(|metadata| metadata.get("superseded_by"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if n.effective_status() == "superseded" {
                match replacement {
                    None => {
                        if !findings
                            .iter()
                            .any(|f| f.kind == "superseded_without_replacement" && f.path == n.path)
                        {
                            findings.push(finding(
                                "superseded_without_replacement",
                                &n.path,
                                None,
                                "status is `superseded` but `superseded_by` is not set".to_string(),
                                "FACT",
                            ));
                        }
                    }
                    Some(target) if !decision_ids.contains(target) => findings.push(finding(
                        "dangling_superseded_by_reference",
                        &n.path,
                        None,
                        format!("`superseded_by` references missing decision '{target}'"),
                        "FACT",
                    )),
                    Some(target) => {
                        let reciprocal = graph.of_type("decision").into_iter().any(|candidate| {
                            candidate.id == target
                                && candidate.supersedes.iter().any(|id| id == &n.id)
                        });
                        if !reciprocal {
                            findings.push(finding(
                                "inconsistent_supersession_reference",
                                &n.path,
                                None,
                                format!("replacement decision '{target}' does not list '{}' in `supersedes`", n.id),
                                "FACT",
                            ));
                        }
                    }
                }
            }
        }
        // Report only actual cycle members, not decisions that merely lead into a cycle. IDs
        // must be unique before they can be used as unambiguous graph vertices.
        let unique_decisions: std::collections::HashMap<&str, &crate::memory::graph::GraphNote> =
            graph
                .of_type("decision")
                .into_iter()
                .filter(|decision| {
                    by_id
                        .get(decision.id.as_str())
                        .is_some_and(|notes| notes.len() == 1)
                })
                .map(|decision| (decision.id.as_str(), decision))
                .collect();
        let mut cycle_members = std::collections::BTreeSet::new();
        for start in unique_decisions.keys().copied() {
            let mut seen = std::collections::HashMap::new();
            let mut path = Vec::new();
            let mut current = start;
            while let Some(decision) = unique_decisions.get(current) {
                if let Some(&cycle_start) = seen.get(current) {
                    cycle_members.extend(path[cycle_start..].iter().copied());
                    break;
                }
                seen.insert(current, path.len());
                path.push(current);
                let Some(next) = decision
                    .supersedes
                    .iter()
                    .find(|id| unique_decisions.contains_key(id.as_str()))
                else {
                    break;
                };
                current = next;
            }
        }
        if !cycle_members.is_empty() {
            findings.push(finding(
                "supersession_cycle",
                "(graph)",
                None,
                format!(
                    "decisions form supersession cycle(s): {}",
                    cycle_members.into_iter().collect::<Vec<_>>().join(", ")
                ),
                "FACT",
            ));
        }
        if let Some(scope) = scope.as_deref() {
            let prefix = format!("{scope}/");
            findings.retain(|finding| finding.path == scope || finding.path.starts_with(&prefix));
        }
        findings.sort_by(|a, b| {
            (a.path.as_str(), a.line, a.kind.as_str()).cmp(&(
                b.path.as_str(),
                b.line,
                b.kind.as_str(),
            ))
        });
        let mut summary = BTreeMap::new();
        for f in &findings {
            *summary.entry(f.kind.clone()).or_insert(0) += 1;
        }
        Ok(HygieneReport {
            checked: docs.len(),
            findings,
            summary,
        })
    }
}
