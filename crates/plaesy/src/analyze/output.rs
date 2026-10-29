//! The analysis artifacts: `project.json`, `project.structure.json` and `overview.md`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use chrono::Local;
use serde::Serialize;

use super::detect::FrameworkHit;
use super::insights::{generate_ai_insights, AiInsights};
use super::walk::{FileInfo, WalkResult};

pub const SOURCE_CODE_EXTS: &[&str] = &[
    "js", "jsx", "ts", "tsx", "py", "go", "java", "dart", "sh", "bash", "ps1", "psm1", "rb", "php",
    "rs", "c", "cc", "cpp", "h", "hpp", "cs", "kt", "kts", "swift",
];
pub const DOC_EXTS: &[&str] = &["md", "txt", "rst", "adoc"];
pub const CFG_EXTS: &[&str] = &["json", "yaml", "yml", "toml", "ini", "xml", "cfg"];

/// Files per bucket, from the extension taxonomy above.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileCounts {
    pub source_code: usize,
    pub documentation: usize,
    pub configuration: usize,
}

/// Everything one run has computed, shared by the artifact writers.
pub struct RunContext<'a> {
    pub project: &'a Path,
    pub walk: &'a WalkResult,
    pub frameworks: &'a [FrameworkHit],
    pub project_type: &'a FrameworkHit,
    pub languages: &'a [String],
    pub dev_tools: &'a [String],
    pub build_systems: &'a [String],
    pub file_counts: FileCounts,
    pub framework_version: &'a str,
}

fn ends_with_any(name: &str, exts: &[&str]) -> bool {
    exts.iter().any(|e| name.ends_with(&format!(".{e}")))
}

/// Classifies each file by extension into one bucket. The first bucket that matches wins.
#[must_use]
pub fn count_file_types(files: &[FileInfo]) -> FileCounts {
    let mut fc = FileCounts::default();
    for f in files {
        let name = f
            .rel_path
            .rsplit('/')
            .next()
            .unwrap_or(&f.rel_path)
            .to_lowercase();
        if ends_with_any(&name, SOURCE_CODE_EXTS) {
            fc.source_code += 1;
        } else if ends_with_any(&name, DOC_EXTS) {
            fc.documentation += 1;
        } else if ends_with_any(&name, CFG_EXTS) {
            fc.configuration += 1;
        }
    }
    fc
}

#[must_use]
pub fn directory_description(rel: &str) -> &'static str {
    match rel {
        "lib" => "Source code library directory",
        "test" => "Test files and unit tests",
        "android" => "Android platform specific code",
        "ios" => "iOS platform specific code",
        "assets" => "Static assets (images, fonts, etc.)",
        "assets/images" => "Image assets",
        "assets/data" => "Data files",
        "lib/models" => "Data models and entities",
        "lib/services" => "Business logic and API services",
        "lib/utils" => "Utility functions and helpers",
        "docs" => "Documentation files",
        "scripts" => "Build and utility scripts",
        _ if rel.starts_with("lib/screens") => "UI screen components",
        _ => "Project directory",
    }
}

#[must_use]
pub fn file_type(rel: &str) -> &'static str {
    if rel.ends_with(".dart") {
        "dart"
    } else if rel.ends_with(".js") || rel.ends_with(".jsx") {
        "javascript"
    } else if rel.ends_with(".ts") || rel.ends_with(".tsx") {
        "typescript"
    } else if rel.ends_with(".py") {
        "python"
    } else if rel.ends_with(".go") {
        "go"
    } else if rel.ends_with(".java") {
        "java"
    } else if rel.ends_with(".json") {
        "json"
    } else if rel.ends_with(".yaml") || rel.ends_with(".yml") {
        "yaml"
    } else if rel.ends_with(".md") {
        "markdown"
    } else {
        "text"
    }
}

#[must_use]
pub fn file_purpose(rel: &str) -> &'static str {
    let base = rel.rsplit('/').next().unwrap_or(rel);
    if base == "pubspec.yaml" {
        "Flutter/Dart project configuration"
    } else if base == "package.json" {
        "Node.js project configuration"
    } else if base == "requirements.txt" {
        "Python dependencies"
    } else if base == "go.mod" {
        "Go module configuration"
    } else if base == "Cargo.toml" {
        "Rust project configuration"
    } else if base.starts_with("README") {
        "Project documentation"
    } else if base == "LICENSE" {
        "Project license"
    } else if base.starts_with("main.") || base.starts_with("index.") {
        "Application entry point"
    } else if base.contains(".config.") || base.ends_with(".config") {
        "Configuration file"
    } else if base == ".gitignore" {
        "Git ignore rules"
    } else if base.starts_with(".env") {
        "Environment variables"
    } else {
        "Project file"
    }
}

/// Files the structure report lists as key files.
#[must_use]
pub fn is_key_file(rel: &str) -> bool {
    let base = rel.rsplit('/').next().unwrap_or(rel);
    matches!(
        base,
        "package.json"
            | "pubspec.yaml"
            | "requirements.txt"
            | "go.mod"
            | "Cargo.toml"
            | "main.dart"
            | "main.js"
            | "main.py"
            | "main.go"
            | "index.js"
    ) || base.ends_with(".md")
        || base.starts_with("README")
        || base == "LICENSE"
        || base.ends_with(".json")
        || base.ends_with(".yaml")
        || base.ends_with(".yml")
}

/// The multi-framework label for `project.json`.
#[must_use]
pub fn project_classification(frameworks: &[FrameworkHit]) -> &'static str {
    if frameworks.len() <= 1 {
        return "single";
    }
    let (mut frontend, mut backend, mut mobile, mut devops) = (false, false, false, false);
    for h in frameworks {
        match h.framework.as_str() {
            "nextjs" | "react" | "vue" | "nuxt" | "svelte" | "astro" | "remix" | "gatsby"
            | "express" | "nestjs" => frontend = true,
            "django" | "flask" | "fastapi" | "laravel" | "rails" | "springboot" | "actix"
            | "rocket" | "gin" | "echo" | "fiber" => {
                backend = true;
            }
            "flutter" => mobile = true,
            "docker" | "docker-compose" | "terraform" | "kustomize" | "vagrant" => devops = true,
            _ => {}
        }
    }
    if frontend && backend {
        "full-stack"
    } else if devops && (frontend || backend) {
        "full-stack-with-infrastructure"
    } else if devops {
        "infrastructure-focused"
    } else if mobile && (backend || frontend) {
        "mobile-backend"
    } else {
        "multi-framework"
    }
}

#[must_use]
pub fn complexity_for(total_files: usize) -> &'static str {
    if total_files > 50 {
        "Large"
    } else if total_files > 20 {
        "Medium"
    } else {
        "Small"
    }
}

#[must_use]
pub fn timestamp() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

fn project_name(project: &Path) -> String {
    project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Writes `value` as JSON indented with four spaces, without a trailing newline.
///
/// # Errors
///
/// The value cannot be serialised or the file cannot be written.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser).map_err(io::Error::other)?;
    fs::write(path, buf)
}

#[derive(Serialize)]
struct ProjectSummary {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    confidence: String,
    description: String,
    purpose: String,
    complexity: String,
    classification: String,
    total_files: usize,
    analysis_timestamp: String,
}

#[derive(Serialize)]
struct TechnologyStack {
    primary_languages: Vec<String>,
    frameworks: Vec<String>,
    development_tools: Vec<String>,
    build_systems: Vec<String>,
}

#[derive(Serialize)]
struct StructureFiles {
    total: usize,
    code: usize,
    documentation: usize,
    configuration: usize,
}

#[derive(Serialize)]
struct StructureSummary {
    files: StructureFiles,
}

#[derive(Serialize)]
struct AnalysisFiles {
    project_structure: &'static str,
    context_file: &'static str,
    overview: &'static str,
    generated_by: String,
}

#[derive(Serialize)]
struct ProjectJson {
    project_summary: ProjectSummary,
    frameworks_detected: Vec<FrameworkHit>,
    ai_insights: AiInsights,
    technology_stack: TechnologyStack,
    structure: StructureSummary,
    framework_version: String,
    analysis_files: AnalysisFiles,
}

/// Writes `project.json`: the summary, frameworks, insights and stack.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_project_json(analysis_dir: &Path, ctx: &RunContext<'_>) -> io::Result<()> {
    let fc = ctx.file_counts;
    let total_files = ctx.walk.files.len();
    let insights = generate_ai_insights(&ctx.project_type.framework, total_files);
    let doc = ProjectJson {
        project_summary: ProjectSummary {
            name: project_name(ctx.project),
            kind: ctx.project_type.framework.clone(),
            confidence: ctx.project_type.confidence.clone(),
            description: insights.overview.clone(),
            purpose: "AI-optimized development project".to_owned(),
            complexity: complexity_for(total_files).to_owned(),
            classification: project_classification(ctx.frameworks).to_owned(),
            total_files,
            analysis_timestamp: timestamp(),
        },
        frameworks_detected: ctx.frameworks.to_vec(),
        ai_insights: insights,
        technology_stack: TechnologyStack {
            primary_languages: ctx.languages.to_vec(),
            frameworks: ctx.frameworks.iter().map(|h| h.framework.clone()).collect(),
            development_tools: ctx.dev_tools.to_vec(),
            build_systems: ctx.build_systems.to_vec(),
        },
        structure: StructureSummary {
            files: StructureFiles {
                total: total_files,
                code: fc.source_code,
                documentation: fc.documentation,
                configuration: fc.configuration,
            },
        },
        framework_version: ctx.framework_version.to_owned(),
        analysis_files: AnalysisFiles {
            project_structure: "project.structure.json",
            context_file: "../context.md",
            overview: "overview.md",
            generated_by: format!("Plaesy Spec-Kit v{}", ctx.framework_version),
        },
    };
    write_json(&analysis_dir.join("project.json"), &doc)
}

#[derive(Serialize)]
struct DirEntry {
    file_count: usize,
    description: &'static str,
}

#[derive(Serialize)]
struct KeyFileEntry {
    size_bytes: u64,
    #[serde(rename = "type")]
    kind: &'static str,
    purpose: &'static str,
}

#[derive(Serialize)]
struct FileTypeCounts {
    source_code: usize,
    documentation: usize,
    configuration: usize,
}

#[derive(Serialize)]
struct RelatedFiles {
    project_summary: &'static str,
    context_file: &'static str,
    overview: &'static str,
}

#[derive(Serialize)]
struct ProjectStructureJson {
    directories: BTreeMap<String, DirEntry>,
    key_files: BTreeMap<String, KeyFileEntry>,
    file_types: FileTypeCounts,
    analysis_timestamp: String,
    related_files: RelatedFiles,
    generated_by: String,
}

/// Writes `project.structure.json`: directories, key files and file-type counts.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_project_structure_json(analysis_dir: &Path, ctx: &RunContext<'_>) -> io::Result<()> {
    let directories = ctx
        .walk
        .dirs
        .iter()
        .map(|d| {
            (
                d.rel_path.clone(),
                DirEntry {
                    file_count: d.file_count,
                    description: directory_description(&d.rel_path),
                },
            )
        })
        .collect();
    let key_files = ctx
        .walk
        .files
        .iter()
        .filter(|f| is_key_file(&f.rel_path))
        .map(|f| {
            (
                f.rel_path.clone(),
                KeyFileEntry {
                    size_bytes: f.size,
                    kind: file_type(&f.rel_path),
                    purpose: file_purpose(&f.rel_path),
                },
            )
        })
        .collect();
    let fc = ctx.file_counts;
    let doc = ProjectStructureJson {
        directories,
        key_files,
        file_types: FileTypeCounts {
            source_code: fc.source_code,
            documentation: fc.documentation,
            configuration: fc.configuration,
        },
        analysis_timestamp: timestamp(),
        related_files: RelatedFiles {
            project_summary: "project.json",
            context_file: "../context.md",
            overview: "overview.md",
        },
        generated_by: format!("Plaesy Spec-Kit v{}", ctx.framework_version),
    };
    write_json(&analysis_dir.join("project.structure.json"), &doc)
}

fn has_test_dir(dirs: &[super::walk::DirInfo]) -> bool {
    dirs.iter().any(|d| {
        let base = d
            .rel_path
            .rsplit('/')
            .next()
            .unwrap_or(&d.rel_path)
            .to_lowercase();
        base.starts_with("test") || base.starts_with("spec") || base == "__tests__"
    })
}

fn has_ci_config(project: &Path) -> bool {
    project.join(".github").join("workflows").is_dir()
        || project.join(".gitlab-ci.yml").is_file()
        || project.join(".travis.yml").is_file()
}

/// Writes `overview.md`, replacing any earlier one.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_overview(analysis_dir: &Path, ctx: &RunContext<'_>) -> io::Result<()> {
    use std::fmt::Write as _;

    let project_desc = if ctx.project_type.framework == "spec-kit" {
        "Plaesy Spec-Kit framework for AI-assisted development workflow automation"
    } else {
        "This is an AI-generated project context document for development assistance"
    };
    let fc = ctx.file_counts;

    let mut recs: Vec<String> = Vec::new();
    if fc.documentation < 3 {
        recs.push(format!(
            "- Add comprehensive documentation (only {} doc file(s) found)",
            fc.documentation
        ));
    }
    if !has_test_dir(&ctx.walk.dirs) {
        recs.push("- Implement automated testing (no test/spec directory detected)".to_owned());
    }
    if !has_ci_config(ctx.project) {
        recs.push("- Set up CI/CD pipeline (no CI configuration detected)".to_owned());
    }
    if recs.is_empty() {
        recs.push(
            "- No gaps detected against baseline checks (docs, tests, CI) — keep it up.".to_owned(),
        );
    }

    let mut b = String::new();
    let _ = write!(b, "# Project Analysis Overview\n\n");
    let _ = writeln!(
        b,
        "*This file is regenerated (replaced, not appended) on every `plaesy analyze` run."
    );
    let _ = write!(b, "Manual notes belong in `.plaesy/context.md` / `.plaesy/memory.md`, which analyze no longer touches.*\n\n");
    let _ = writeln!(b, "**Generated**: {}", timestamp());
    let _ = writeln!(b, "**Project**: {}", project_name(ctx.project));
    let _ = write!(b, "**Description**: {project_desc}\n\n");
    let _ = writeln!(b, "## Technology Stack");
    let _ = writeln!(b, "- **Languages**: {}", ctx.languages.join(", "));
    let _ = writeln!(b, "- **Framework**: {}", ctx.project_type.framework);
    let _ = write!(b, "- **Tools**: Git, Plaesy CLI\n\n");
    let _ = writeln!(b, "## File Structure");
    let _ = writeln!(b, "| File Type | Count |");
    let _ = writeln!(b, "|-----------|-------|");
    let _ = writeln!(b, "| Source Code | {} |", fc.source_code);
    let _ = writeln!(b, "| Documentation | {} |", fc.documentation);
    let _ = write!(b, "| Config | {} |\n\n", fc.configuration);
    let _ = write!(b, "## Recommendations\n{}\n\n", recs.join("\n"));
    let _ = writeln!(b, "## Related Analysis Files");
    let _ = writeln!(
        b,
        "- **Project Summary**: `project.json` - Complete project overview and AI insights"
    );
    let _ = write!(b, "- **Project Structure**: `project.structure.json` - Detailed file and directory analysis\n\n");
    let _ = write!(
        b,
        "---\n*Generated by Plaesy Spec-Kit v{}*\n",
        ctx.framework_version
    );

    fs::write(analysis_dir.join("overview.md"), b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_counts_once_in_the_first_matching_bucket() {
        let files = vec![
            FileInfo {
                rel_path: "src/main.rs".into(),
                size: 0,
                mod_unix: 0,
            },
            FileInfo {
                rel_path: "README.MD".into(),
                size: 0,
                mod_unix: 0,
            },
            FileInfo {
                rel_path: "Cargo.toml".into(),
                size: 0,
                mod_unix: 0,
            },
        ];
        let fc = count_file_types(&files);
        assert_eq!(
            (fc.source_code, fc.documentation, fc.configuration),
            (1, 1, 1)
        );
    }

    #[test]
    fn classification_needs_frontend_and_backend_for_full_stack() {
        let hit = |n: &str| FrameworkHit {
            framework: n.to_owned(),
            confidence: "high".to_owned(),
        };
        assert_eq!(
            project_classification(&[hit("react"), hit("django")]),
            "full-stack"
        );
        assert_eq!(
            project_classification(&[hit("react"), hit("go")]),
            "multi-framework"
        );
    }
}
