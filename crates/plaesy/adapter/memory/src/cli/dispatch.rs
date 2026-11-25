//! Command dispatch for `plaesy memory`, run after configuration and engine setup.

use super::*;

pub(super) async fn dispatch(
    engine: SearchEngine,
    config: MemoryConfig,
    command: Command,
) -> Result<()> {
    match command {
        Command::Config { sub: _ }
        | Command::Serve { .. }
        | Command::Onboard { .. }
        | Command::CodeFind { .. }
        | Command::CodeAudit { .. } => {
            unreachable!("handled above")
        }
        Command::Schedule { mode } => {
            let jobs = crate::memory::schedule::jobs_from_config(&config.schedule)?;
            let root = config.watch_dir.clone();
            require_dir(&root)?;
            let log = crate::memory::schedule::log_path();
            match mode {
                ScheduleMode::Once => {
                    let now = chrono::Utc::now();
                    for o in
                        crate::memory::schedule::run_due(&engine, &root, &jobs, now, log.as_deref())
                            .await?
                    {
                        println!("{}", o.line(now));
                    }
                }
                ScheduleMode::List => {
                    for job in &jobs {
                        println!("{}	every {}s	{:?}", job.name, job.every.num_seconds(), job.action);
                    }
                }
                ScheduleMode::Run => {
                    crate::memory::schedule::run_loop(&engine, &root, &jobs, log.as_deref())
                        .await?;
                }
                ScheduleMode::Uninstall => {
                    unreachable!("schedule uninstall is handled before configuration load")
                }
                ScheduleMode::Install => {
                    unreachable!("schedule registration modes are handled before engine creation")
                }
            }
        }
        Command::Index { dir } => {
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            require_dir(&root)?;
            let s = engine.index_directory(&root).await?;
            println!(
                "indexed {} | unchanged {} | removed {} | empty skipped {} | chunks written {} | failed {}",
                s.indexed,
                s.unchanged,
                s.removed,
                s.skipped_empty,
                s.chunks,
                s.failed.len()
            );
            for (path, error) in &s.failed {
                eprintln!("  failed: {path}: {error}");
            }
            if !s.failed.is_empty() {
                std::process::exit(2);
            }
        }
        Command::Fingerprint { path } => {
            let hash = engine.fingerprint_note(&path)?;
            println!("{hash}  {}", path.display());
        }
        Command::Search {
            query,
            limit,
            mode,
            format,
            tiers,
            path,
            min_trust,
            include_inactive,
            as_of,
            expand_links,
            explain,
        } => {
            for t in &tiers {
                if normalize_tier(t).is_none() {
                    return Err(MemoryError::ConfigError(format!(
                        "unknown tier '{t}' (expected one of: {})",
                        TIERS.join(", ")
                    )));
                }
            }
            let min_trust = match min_trust {
                None => None,
                Some(t) => Some(
                    Trust::parse(&serde_json::Value::String(t.clone())).ok_or_else(|| {
                        MemoryError::ConfigError(format!(
                            "unknown trust '{t}' (expected high, medium or low)"
                        ))
                    })?,
                ),
            };
            let as_of = as_of
                .map(|s| {
                    crate::memory::search::parse_date(&s).ok_or_else(|| {
                        MemoryError::ConfigError(format!(
                            "--as-of '{s}' is not a date (expected YYYY-MM-DD)"
                        ))
                    })
                })
                .transpose()?;
            let filter = SearchFilter {
                tiers,
                path_prefix: path,
                min_trust,
                include_inactive,
                as_of,
                ..SearchFilter::default()
            };
            let mode = match mode {
                Mode::Hybrid => SearchMode::Hybrid,
                Mode::Semantic => SearchMode::Semantic,
                Mode::Keyword => SearchMode::Keyword,
            };
            let results = if expand_links {
                engine
                    .search_expanded_with_explain(&query, limit, mode, &filter, explain)
                    .await?
            } else {
                engine
                    .search_with_explain(&query, limit, mode, &filter, explain)
                    .await?
            };
            engine.note_query(&query, &results)?;
            match format {
                OutputFormat::Json => {
                    let hits: Vec<SearchHit> = results
                        .iter()
                        .map(SearchHit::from_result)
                        .collect::<Result<_>>()?;
                    let out = serde_json::to_string_pretty(&hits)
                        .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                    println!("{out}");
                }
                OutputFormat::Md => {
                    if results.is_empty() {
                        println!("no results");
                    } else {
                        for r in &results {
                            let c = CitationBuilder::new().build(r)?;
                            println!("- {c}");
                        }
                    }
                }
                OutputFormat::Text => {
                    if results.is_empty() {
                        println!("no results");
                    } else {
                        let b = CitationBuilder::new();
                        for (i, r) in results.iter().enumerate() {
                            println!("{}. score {:.3}  {}", i + 1, r.score, b.build(r)?);
                            if explain {
                                if let Some(ex) = &r.explain {
                                    print!("     explain: fused={:.4}", ex.fused.unwrap_or(0.0));
                                    if let Some(v) = ex.fts {
                                        print!(" fts={:.4}", v);
                                    }
                                    if let Some(v) = ex.vector {
                                        print!(" vector={:.4}", v);
                                    }
                                    if let Some(v) = ex.staleness_factor {
                                        print!(" staleness={:.1}", v);
                                    }
                                    println!();
                                }
                            }
                            for line in r.text.lines().take(4) {
                                println!("     {line}");
                            }
                        }
                    }
                }
            }
        }
        Command::Links { note, outgoing } => {
            let links = engine.links(&note, !outgoing).await?;
            if links.is_empty() {
                println!("no links");
            }
            for l in links {
                println!(
                    "{}:{}  [[{}]] -> {}",
                    l.source_path,
                    l.line,
                    l.target,
                    l.resolved_path.as_deref().unwrap_or("(unresolved)")
                );
            }
        }
        Command::Hygiene {
            dir,
            project_roots,
            json,
            strict,
            strict_facts,
        } => {
            let vault_root =
                config
                    .watch_dir
                    .canonicalize()
                    .map_err(|source| MemoryError::IoError {
                        path: config.watch_dir.display().to_string(),
                        source,
                    })?;
            let requested_scope = dir.unwrap_or_else(|| vault_root.clone());
            let scope = requested_scope
                .canonicalize()
                .map_err(|source| MemoryError::IoError {
                    path: requested_scope.display().to_string(),
                    source,
                })?;
            if !scope.is_dir() || !scope.starts_with(&vault_root) {
                return Err(MemoryError::ConfigError(format!(
                    "hygiene directory must be inside the configured vault: {}",
                    scope.display()
                )));
            }
            let relative_scope = scope
                .strip_prefix(&vault_root)
                .map_err(|_| MemoryError::ConfigError("invalid hygiene scope".into()))?;
            let project_roots = parse_project_roots(&project_roots)?;
            let report = engine
                .hygiene(&HygieneOptions {
                    root: Some(vault_root),
                    scope: (!relative_scope.as_os_str().is_empty())
                        .then(|| relative_scope.to_path_buf()),
                    project_roots,
                    ..HygieneOptions::default()
                })
                .await?;
            if json {
                let out = serde_json::to_string_pretty(&report)
                    .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                println!("{out}");
            } else {
                println!(
                    "{} notes checked, {} findings",
                    report.checked,
                    report.findings.len()
                );
                for f in &report.findings {
                    let at = f.line.map(|l| format!(":{l}")).unwrap_or_default();
                    println!(
                        "  [{}] {:<30} {}{at}  {}",
                        f.label, f.kind, f.path, f.detail
                    );
                }
                for (kind, n) in &report.summary {
                    println!("  {kind}: {n}");
                }
            }
            if hygiene_strict_failure(&report, strict, strict_facts) {
                std::process::exit(1);
            }
        }
        Command::Curate {
            dir,
            project_roots,
            limit,
            json,
        } => {
            if let Some(root) = &dir {
                require_dir(root)?;
            }
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            require_dir(&root)?;
            let project_roots = parse_project_roots(&project_roots)?;
            if WriteLock::daemon_is_active(&root) {
                eprintln!("Using the index maintained by the active memory daemon.");
            } else {
                engine.index_directory(&root).await?;
            }
            let report = engine
                .curate(
                    &HygieneOptions {
                        root: Some(root),
                        project_roots,
                        ..HygieneOptions::default()
                    },
                    limit,
                )
                .await?;
            if json {
                let out = serde_json::to_string_pretty(&report)
                    .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                println!("{out}");
            } else {
                print_curation_report(&report);
            }
        }
        Command::Missing {
            resolve,
            limit,
            json,
            all,
            dir,
        } => {
            if let Some(id) = resolve {
                if engine.resolve_gap(id)? {
                    println!("resolved gap {id}");
                } else {
                    println!("no open gap with id {id}");
                }
                return Ok(());
            }
            if let Some(d) = &dir {
                if !d.is_dir() {
                    return Err(MemoryError::ConfigError(format!(
                        "not a directory: {}",
                        d.display()
                    )));
                }
            }
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            if root.is_dir() {
                engine.index_directory(&root).await?;
            }
            let report = engine.missing(root.is_dir().then_some(root), limit).await?;
            if json {
                let out = serde_json::to_string_pretty(&report)
                    .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
                println!("{out}");
            } else if report.query_gaps.is_empty() && report.structural.is_empty() {
                println!("no gaps");
            }
            if !json {
                for g in &report.query_gaps {
                    let score = g
                        .last_top_score
                        .map_or("no hit".to_string(), |s| format!("top {s:.2}"));
                    // One line per gap, whatever the query held.
                    let flat = g.query.split_whitespace().collect::<Vec<_>>().join(" ");
                    let shown: String = flat.chars().take(80).collect();
                    let ellipsis = if flat.chars().count() > 80 { "…" } else { "" };
                    println!(
                        "  #{:<4} pri {:>5.2}  x{:<3} {score:<10} {shown}{ellipsis}",
                        g.id, g.priority, g.misses
                    );
                }
                // Structural gaps can run to hundreds of lines: show a count per kind and a
                // sample of each, unless --all asks for everything.
                let mut shown: std::collections::BTreeMap<&str, usize> = Default::default();
                for f in &report.structural {
                    let n = shown.entry(f.kind.as_str()).or_insert(0);
                    *n += 1;
                    if !all && *n > SAMPLE_PER_KIND {
                        continue;
                    }
                    let at = f.line.map(|l| format!(":{l}")).unwrap_or_default();
                    println!(
                        "  [{}] {:<14} {}{at}  {}",
                        f.label, f.kind, f.path, f.detail
                    );
                }
                let total: usize = report.structural.len();
                if !report.structural.is_empty() {
                    println!("structural gaps: {total}");
                    for (kind, n) in &shown {
                        if !all && *n > SAMPLE_PER_KIND {
                            println!("  {kind}: {n} (showing {SAMPLE_PER_KIND}, use --all)");
                        } else {
                            println!("  {kind}: {n}");
                        }
                    }
                }
            }
        }
        Command::List {
            offset,
            limit,
            note_type,
            status,
            query,
            path,
            format,
        } => {
            if limit == 0 || limit > 1000 {
                return Err(MemoryError::ConfigError(
                    "list limit must be between 1 and 1000".into(),
                ));
            }
            let database = engine.database();
            let total = database
                .count_documents(
                    path.as_deref(),
                    note_type.as_deref(),
                    status.as_deref(),
                    query.as_deref(),
                )
                .await?;
            let page = database
                .list_documents(
                    path.as_deref(),
                    note_type.as_deref(),
                    status.as_deref(),
                    query.as_deref(),
                    limit,
                    offset,
                )
                .await?;
            match format {
                GraphFormat::Json => {
                    let items: Vec<_> = page.iter().map(|doc| serde_json::json!({
                        "path": doc.path,
                        "title": doc.path.rsplit('/').next().unwrap_or(&doc.path),
                        "type": doc.metadata.get("type").or_else(|| doc.metadata.get("note_type")),
                        "project": doc.metadata.get("project"),
                        "scope": doc.metadata.get("scope"),
                        "tags": doc.metadata.get("tags"),
                    })).collect();
                    println!(
                        "{}",
                        serde_json::json!({"total": total, "offset": offset, "limit": limit, "items": items})
                    );
                }
                GraphFormat::Text => {
                    for doc in &page {
                        let kind = doc
                            .metadata
                            .get("type")
                            .or_else(|| doc.metadata.get("note_type"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("-");
                        println!("{}\t{}", doc.path, kind);
                    }
                    println!("showing {} of {total} (offset {offset})", page.len());
                }
            }
        }
        Command::History {
            limit,
            operation,
            json,
        } => {
            if !(1..=1000).contains(&limit) {
                return Err(MemoryError::ConfigError(
                    "history limit must be between 1 and 1000".into(),
                ));
            }
            let events =
                crate::memory::lifecycle::history(&config.watch_dir, limit, operation.as_deref())?;
            if json {
                let output = serde_json::to_string_pretty(&events)
                    .map_err(|error| MemoryError::SerializationError(error.to_string()))?;
                println!("{output}");
            } else if events.is_empty() {
                println!("no lifecycle events");
            } else {
                for event in events {
                    println!(
                        "{}\t{}\t{}\t{} -> {}\t{}",
                        event
                            .get("occurred_at")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("-"),
                        event
                            .get("operation")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("-"),
                        event
                            .get("phase")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("-"),
                        event
                            .get("from_state")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("-"),
                        event
                            .get("to_state")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("-"),
                        event
                            .get("evidence")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("-")
                    );
                }
            }
        }
        Command::Stats => {
            let s = engine.statistics().await?;
            println!("documents:  {}", s.documents_total);
            println!("chunks:     {}", s.chunks_total);
            println!("db size:    {} bytes", s.db_size_bytes);
            println!("fts size:   {} bytes", s.fts_index_size_bytes);
            println!("hnsw size:  {} bytes", s.hnsw_size_bytes);
            println!(
                "vector search: {}",
                if engine.has_vector_search() {
                    "on"
                } else {
                    "off (keyword-only)"
                }
            );
        }
        Command::Watch { dir } => {
            let root = dir.unwrap_or_else(|| config.watch_dir.clone());
            require_dir(&root)?;
            let s = engine.index_directory(&root).await?;
            println!(
                "initial sync: indexed {} unchanged {} removed {}",
                s.indexed, s.unchanged, s.removed
            );
            let mut watcher = Watcher::new(&root.to_string_lossy()).await?;
            println!("watching {} (Ctrl-C to stop)", root.display());
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => break,
                    changes = watcher.changes() => {
                        let events = changes?;
                        if events.is_empty() { break; }
                        let n = engine.apply_events(&root, &events).await;
                        println!("applied {n}/{} file events", events.len());
                    }
                }
            }
        }
        Command::Projects {
            id,
            sub,
            include_inactive,
            format,
        } => {
            let graph = crate::memory::graph::load_graph(&engine.database()).await?;
            match (id, sub) {
                (None, None) => print_projects(&graph, include_inactive, format),
                (None, Some(_)) => {
                    return Err(MemoryError::ConfigError(
                        "a subcommand (decisions/contexts) needs a project id or '*' first, e.g. `memory projects * decisions`".into(),
                    ));
                }
                (Some(id), None) => print_project_detail(&graph, &id, format)?,
                (Some(id), Some(sub)) => print_project_scoped(&graph, &id, sub, format)?,
            }
        }
        Command::Decisions {
            projects,
            status,
            include_inactive,
            limit,
            format,
        } => {
            let graph = crate::memory::graph::load_graph(&engine.database()).await?;
            validate_projects_filter(&graph, &projects)?;
            let filter = crate::memory::graph::ListFilter {
                projects,
                status,
                include_inactive,
                limit,
            };
            print_decision_list(&graph, &filter, format);
        }
        Command::Contexts {
            projects,
            status,
            include_inactive,
            limit,
            format,
        } => {
            let graph = crate::memory::graph::load_graph(&engine.database()).await?;
            validate_projects_filter(&graph, &projects)?;
            let filter = crate::memory::graph::ListFilter {
                projects,
                status,
                include_inactive,
                limit,
            };
            print_context_list(&graph, &filter, format);
        }
        Command::Remember {
            title,
            body,
            folder,
            template,
            values,
            tier,
            confidence,
            note_type,
            id,
            status,
            project,
            scope,
            supersedes,
            json,
        } => {
            let body = match (body, template.is_some()) {
                (Some(body), _) => body,
                (None, true) => String::new(),
                (None, false) => {
                    use std::io::Read;
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf).map_err(|e| {
                        MemoryError::IoError {
                            path: "<stdin>".into(),
                            source: e,
                        }
                    })?;
                    buf
                }
            };
            let has_graph_fields = id.is_some()
                || status.is_some()
                || project.is_some()
                || !scope.is_empty()
                || !supersedes.is_empty();
            if template.is_none() && note_type.is_none() && has_graph_fields {
                return Err(MemoryError::ConfigError(
                    "--id/--status/--project/--scope/--supersedes require --type".into(),
                ));
            }
            if let Some(template_name) = template {
                let explicit_folder = folder.is_some();
                let root = engine.watch_root().ok_or_else(|| {
                    MemoryError::ConfigError("no notes root is configured".into())
                })?;
                let (template_folder, markdown, graph) = crate::memory::notes::render_template(
                    root,
                    &template_name,
                    &title,
                    &body,
                    crate::memory::notes::TemplateOverrides {
                        folder: folder.as_deref(),
                        values: &values,
                        id: id.as_deref(),
                        status: status.as_deref(),
                        project: project.as_deref(),
                        scope: &scope,
                        supersedes: &supersedes,
                    },
                )?;
                if let Some(requested_type) = note_type {
                    if graph.as_ref().map(|g| g.note_type.as_str()) != Some(requested_type.as_str())
                    {
                        return Err(MemoryError::ConfigError(format!(
                            "--type '{requested_type}' does not match template '{template_name}'"
                        )));
                    }
                }
                if graph.is_none() && has_graph_fields {
                    return Err(MemoryError::ConfigError(format!(
                        "template '{template_name}' is not a graph type; remove --id/--status/--project/--scope/--supersedes"
                    )));
                }
                let note = crate::memory::notes::NewNote {
                    folder: template_folder,
                    title: if !explicit_folder
                        && graph.as_ref().is_some_and(|g| g.note_type == "project")
                    {
                        "README".into()
                    } else {
                        title
                    },
                    body,
                    tier,
                    confidence,
                    graph,
                };
                let path = engine.remember_rendered(&note, &markdown).await?;
                if json {
                    println!(
                        "{}",
                        serde_json::json!({ "path": path.display().to_string() })
                    );
                } else {
                    println!("saved: {}", path.display());
                }
                return Ok(());
            }
            if !values.is_empty() {
                return Err(MemoryError::ConfigError("--set requires --template".into()));
            }
            let graph = note_type
                .as_ref()
                .map(|note_type| crate::memory::notes::GraphFields {
                    note_type: note_type.clone(),
                    id: id.clone().unwrap_or_default(),
                    status,
                    project,
                    scope,
                    supersedes,
                });
            let note = crate::memory::notes::NewNote {
                folder: folder.unwrap_or_default(),
                title,
                body,
                tier,
                confidence,
                graph,
            };
            let path = engine.remember(&note).await?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "path": path.display().to_string() })
                );
            } else {
                println!("saved: {}", path.display());
            }
        }
        Command::Forget { path, reason, yes } => {
            if !yes {
                return Err(MemoryError::ConfigError(
                    "forget permanently removes the source file; rerun with --yes after reviewing the path".into(),
                ));
            }
            let shown = path.display().to_string();
            engine.forget_note(&path, &reason).await?;
            println!("forgotten: {shown}");
        }
        Command::Update {
            path,
            expected_sha256,
            reason,
            yes: _,
        } => {
            use std::io::Read;
            let mut updated = String::new();
            std::io::stdin()
                .read_to_string(&mut updated)
                .map_err(|source| MemoryError::IoError {
                    path: "<stdin>".into(),
                    source,
                })?;
            let hash = engine
                .update_note(&path, &expected_sha256, &updated, &reason)
                .await?;
            println!("updated: {} (sha256 {hash})", path.display());
        }
        Command::Archive {
            path,
            expected_sha256,
            reason,
            yes: _,
        } => {
            let hash = engine
                .archive_note(&path, &expected_sha256, &reason)
                .await?;
            println!("archived: {} (sha256 {hash})", path.display());
        }
    }
    if engine.has_vector_search() {
        engine.save_snapshot().await;
    }
    Ok(())
}
