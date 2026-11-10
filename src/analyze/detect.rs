//! Framework, language, tool and build-system detection from marker files and their text.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// One detected framework and how sure the detector is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameworkHit {
    pub framework: String,
    pub confidence: String,
}

fn file_exists(path: &Path) -> bool {
    path.is_file()
}

fn dir_exists(path: &Path) -> bool {
    path.is_dir()
}

/// True when any of `names` (project-relative) is an existing file.
fn any_file(project: &Path, names: &[&str]) -> bool {
    names.iter().any(|n| file_exists(&project.join(n)))
}

fn is_glob(pattern: &str) -> bool {
    pattern.contains(['*', '?', '['])
}

/// Entries of `dir` matching a `*` pattern such as `*.csproj`. Other patterns match nothing.
fn glob(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
        })
        .collect()
}

fn any_file_matches(dir: &Path, pattern: &str) -> bool {
    !glob(dir, pattern).is_empty()
}

/// True when one of the files named by `patterns` (relative to `dir`) contains `needle`.
/// A missing or unreadable file counts as no match.
fn file_contains(dir: &Path, needle: &str, patterns: &[&str]) -> bool {
    for pattern in patterns {
        let paths = if is_glob(pattern) {
            glob(dir, pattern)
        } else {
            vec![dir.join(pattern)]
        };
        for p in paths {
            if let Ok(data) = fs::read(&p) {
                if String::from_utf8_lossy(&data).contains(needle) {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Default)]
struct Hits(Vec<FrameworkHit>);

impl Hits {
    fn add(&mut self, name: &str, confidence: &str) {
        if !self.0.iter().any(|h| h.framework == name) {
            self.0.push(FrameworkHit {
                framework: name.to_owned(),
                confidence: confidence.to_owned(),
            });
        }
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Every framework the project's marker files point at.
#[must_use]
pub fn detect_all_frameworks(project: &Path) -> Vec<FrameworkHit> {
    let mut h = Hits::default();

    // JavaScript and TypeScript frameworks.
    if file_exists(&project.join("package.json")) {
        let before = h.len();
        for (needle, name) in [
            ("next", "nextjs"),
            ("react", "react"),
            ("vue", "vue"),
            ("svelte", "svelte"),
            ("astro", "astro"),
            ("remix", "remix"),
            ("gatsby", "gatsby"),
            ("nuxt", "nuxt"),
            ("express", "express"),
            ("nestjs", "nestjs"),
        ] {
            if file_contains(project, needle, &["package.json"]) {
                h.add(name, "high");
            }
        }
        if h.len() == before {
            h.add("nodejs", "medium");
        }
    }
    if any_file(project, &["next.config.js", "next.config.mjs"]) {
        h.add("nextjs", "high");
    }
    if any_file(project, &["nuxt.config.js", "nuxt.config.ts"]) {
        h.add("nuxt", "high");
    }
    if any_file(project, &["svelte.config.js"]) {
        h.add("svelte", "high");
    }
    if any_file(project, &["astro.config.mjs", "astro.config.ts"]) {
        h.add("astro", "high");
    }
    if any_file(project, &["remix.config.js"]) {
        h.add("remix", "high");
    }
    if any_file(project, &["gatsby-config.js"]) {
        h.add("gatsby", "high");
    }
    if any_file(project, &["vite.config.js", "vite.config.ts"]) {
        h.add("vite", "medium");
    }

    // Python frameworks.
    if file_exists(&project.join("pyproject.toml")) {
        let before = h.len();
        if file_contains(project, "django", &["pyproject.toml"]) {
            h.add("django", "high");
        }
        if file_contains(project, "fastapi", &["pyproject.toml"]) {
            h.add("fastapi", "high");
        }
        if file_contains(project, "poetry", &["pyproject.toml"]) {
            h.add("poetry", "medium");
        }
        if h.len() == before {
            h.add("python", "medium");
        }
    }
    if file_exists(&project.join("requirements.txt")) {
        let before = h.len();
        if file_contains(project, "django", &["requirements.txt"]) {
            h.add("django", "high");
        }
        if file_contains(project, "fastapi", &["requirements.txt"]) {
            h.add("fastapi", "high");
        }
        if file_contains(project, "flask", &["requirements.txt"]) {
            h.add("flask", "high");
        }
        if h.len() == before {
            h.add("python", "medium");
        }
    }
    if any_file(project, &["Pipfile"]) {
        h.add("pipenv", "high");
    }
    if any_file(project, &["manage.py"]) {
        h.add("django", "high");
    }

    // Mobile.
    if any_file(project, &["pubspec.yaml"]) {
        h.add("flutter", "high");
    }

    // Go.
    if file_exists(&project.join("go.mod")) {
        let before = h.len();
        if file_contains(project, "gin-gonic", &["go.mod"]) {
            h.add("gin", "high");
        }
        if file_contains(project, "labstack/echo", &["go.mod"]) {
            h.add("echo", "high");
        }
        if file_contains(project, "gofiber", &["go.mod"]) {
            h.add("fiber", "high");
        }
        if h.len() == before {
            h.add("go", "medium");
        }
    }

    // Rust.
    if file_exists(&project.join("Cargo.toml")) {
        let before = h.len();
        if file_contains(project, "actix-web", &["Cargo.toml"]) {
            h.add("actix", "high");
        }
        if file_contains(project, "rocket", &["Cargo.toml"]) {
            h.add("rocket", "high");
        }
        if file_contains(project, "axum", &["Cargo.toml"]) {
            h.add("axum", "high");
        }
        if h.len() == before {
            h.add("rust", "medium");
        }
    }

    // Java (Maven).
    if file_exists(&project.join("pom.xml")) {
        let before = h.len();
        if file_contains(project, "spring-boot", &["pom.xml"]) {
            h.add("springboot", "high");
        }
        if file_contains(project, "spring", &["pom.xml"]) {
            h.add("spring", "high");
        }
        if h.len() == before {
            h.add("maven", "medium");
        }
    }
    // Java (Gradle).
    if any_file(project, &["build.gradle", "build.gradle.kts"]) {
        let before = h.len();
        if file_contains(
            project,
            "org.springframework.boot",
            &["build.gradle", "build.gradle.kts"],
        ) {
            h.add("springboot", "high");
        }
        if file_contains(project, "io.ktor", &["build.gradle", "build.gradle.kts"]) {
            h.add("ktor", "high");
        }
        if h.len() == before {
            h.add("gradle", "medium");
        }
    }

    // Ruby.
    if file_exists(&project.join("Gemfile")) {
        let before = h.len();
        if file_contains(project, "rails", &["Gemfile"]) {
            h.add("rails", "high");
        }
        if file_contains(project, "sinatra", &["Gemfile"]) {
            h.add("sinatra", "high");
        }
        if h.len() == before {
            h.add("ruby", "medium");
        }
    }
    if any_file(project, &["config/application.rb"]) {
        h.add("rails", "high");
    }

    // PHP.
    if file_exists(&project.join("composer.json")) {
        let before = h.len();
        if file_contains(project, "laravel/framework", &["composer.json"]) {
            h.add("laravel", "high");
        }
        if file_contains(project, "symfony", &["composer.json"]) {
            h.add("symfony", "high");
        }
        if h.len() == before {
            h.add("php", "medium");
        }
    }
    if any_file(project, &["wp-config.php"]) {
        h.add("wordpress", "high");
    }

    // C# and .NET.
    if any_file_matches(project, "*.csproj") {
        let before = h.len();
        if file_contains(project, "Microsoft.AspNetCore", &["*.csproj"]) {
            h.add("aspnet", "high");
        }
        if h.len() == before {
            h.add("dotnet", "medium");
        }
    }

    if any_file(project, &["Package.swift"]) {
        h.add("swift", "high");
    }

    // Containers and DevOps.
    if any_file(project, &["Dockerfile"]) {
        h.add("docker", "medium");
    }
    if any_file(project, &["docker-compose.yml", "docker-compose.yaml"]) {
        h.add("docker-compose", "medium");
    }
    if any_file(project, &["Terrafile", "main.tf"]) {
        h.add("terraform", "high");
    }
    if any_file(project, &["Vagrantfile"]) {
        h.add("vagrant", "high");
    }
    if any_file(project, &["kustomization.yaml"]) {
        h.add("kustomize", "high");
    }

    h.0
}

/// Framework names in priority order, used to pick the primary one.
const PRIORITY_FRAMEWORKS: &[&str] = &[
    "nextjs",
    "react",
    "vue",
    "nuxt",
    "svelte",
    "astro",
    "remix",
    "gatsby",
    "django",
    "flask",
    "fastapi",
    "laravel",
    "rails",
    "express",
    "nestjs",
    "springboot",
    "flutter",
    "actix",
    "rocket",
    "gin",
    "echo",
    "fiber",
    "rust",
    "go",
    "java",
    "kotlin",
    "nodejs",
    "python",
    "php",
    "ruby",
    "swift",
];

/// The single primary framework: the only hit, or the first priority match, or the first hit.
#[must_use]
pub fn detect_project_type(hits: &[FrameworkHit]) -> FrameworkHit {
    match hits {
        [] => FrameworkHit {
            framework: "generic".to_owned(),
            confidence: "low".to_owned(),
        },
        [only] => only.clone(),
        _ => PRIORITY_FRAMEWORKS
            .iter()
            .find_map(|p| hits.iter().find(|h| h.framework == *p))
            .unwrap_or(&hits[0])
            .clone(),
    }
}

/// Language name and the extensions that indicate it, in the canonical report order.
const LANGUAGE_DEFS: &[(&str, &[&str])] = &[
    ("JavaScript", &["js", "jsx"]),
    ("TypeScript", &["ts", "tsx"]),
    ("Python", &["py"]),
    ("Go", &["go"]),
    ("Java", &["java"]),
    ("Dart", &["dart"]),
    ("C++", &["cpp", "cc", "cxx"]),
    ("C", &["c"]),
    ("C/C++ Headers", &["h"]),
    ("PHP", &["php"]),
    ("Ruby", &["rb"]),
    ("Rust", &["rs"]),
    ("Swift", &["swift"]),
    ("Kotlin", &["kt"]),
    ("Scala", &["scala"]),
    ("Shell", &["sh"]),
    ("HTML", &["html"]),
    ("CSS", &["css", "scss", "sass"]),
];

/// The languages present among `files` (project-relative slash paths), defaulting to JavaScript.
#[must_use]
pub fn detect_all_languages(files: &[String]) -> Vec<String> {
    let mut out: Vec<String> = LANGUAGE_DEFS
        .iter()
        .filter(|(_, exts)| {
            files.iter().any(|f| {
                let base = f.rsplit('/').next().unwrap_or(f);
                exts.iter().any(|e| base.ends_with(&format!(".{e}")))
            })
        })
        .map(|(name, _)| (*name).to_owned())
        .collect();
    if out.is_empty() {
        out.push("JavaScript".to_owned());
    }
    out
}

fn add_tool(tools: &mut Vec<String>, name: &str) {
    if !tools.iter().any(|t| t == name) {
        tools.push(name.to_owned());
    }
}

/// Version-control, package-manager, CI, test and lint tools the project uses.
#[must_use]
pub fn detect_development_tools(project: &Path) -> Vec<String> {
    let mut tools = Vec::new();
    let p = |name: &str| project.join(name);

    if dir_exists(&p(".git")) {
        add_tool(&mut tools, "Git");
    }
    if dir_exists(&p(".svn")) {
        add_tool(&mut tools, "Subversion");
    }
    if file_exists(&p(".hg")) {
        add_tool(&mut tools, "Mercurial");
    }

    if any_file(
        project,
        &[
            "package.json",
            "package-lock.json",
            "yarn.lock",
            "pnpm-lock.yaml",
        ],
    ) {
        add_tool(&mut tools, "npm/yarn/pnpm");
    }
    if any_file(project, &["requirements.txt", "Pipfile", "pyproject.toml"]) {
        add_tool(&mut tools, "pip/poetry");
    }
    if any_file(project, &["go.mod"]) {
        add_tool(&mut tools, "Go Modules");
    }
    if any_file(project, &["Cargo.toml"]) {
        add_tool(&mut tools, "Cargo");
    }
    if any_file(project, &["pom.xml", "build.gradle", "build.gradle.kts"]) {
        add_tool(&mut tools, "Maven/Gradle");
    }
    if any_file(project, &["Gemfile"]) {
        add_tool(&mut tools, "Bundler");
    }
    if any_file(project, &["composer.json"]) {
        add_tool(&mut tools, "Composer");
    }
    if any_file(project, &["pubspec.yaml"]) {
        add_tool(&mut tools, "Pub");
    }

    if dir_exists(&p(".github").join("workflows")) {
        add_tool(&mut tools, "GitHub Actions");
    }
    if any_file(project, &[".gitlab-ci.yml"]) {
        add_tool(&mut tools, "GitLab CI");
    }
    if any_file(project, &["Jenkinsfile"]) {
        add_tool(&mut tools, "Jenkins");
    }
    if any_file(project, &["azure-pipelines.yml"]) {
        add_tool(&mut tools, "Azure Pipelines");
    }

    if any_file(project, &["jest.config.js", "jest.config.json"])
        || file_contains(project, "jest", &["package.json"])
    {
        add_tool(&mut tools, "Jest");
    }
    if any_file(project, &["vitest.config.js", "vitest.config.ts"]) {
        add_tool(&mut tools, "Vitest");
    }
    if file_contains(project, "pytest", &["requirements.txt"])
        || any_file(project, &["pytest.ini"])
        || dir_exists(&p("tests"))
    {
        add_tool(&mut tools, "pytest");
    }
    if any_file(project, &["go.mod"]) && file_contains(project, "test", &["go.mod"]) {
        add_tool(&mut tools, "Go test");
    }

    if any_file(
        project,
        &[".eslintrc.js", ".eslintrc.json", ".eslintrc.yml"],
    ) || file_contains(project, "eslint", &["package.json"])
    {
        add_tool(&mut tools, "ESLint");
    }
    if any_file(
        project,
        &[".prettierrc", ".prettierrc.json", ".prettierrc.js"],
    ) || file_contains(project, "prettier", &["package.json"])
    {
        add_tool(&mut tools, "Prettier");
    }
    if any_file(project, &["pylintrc", ".flake8"])
        || file_contains(project, "flake8", &["requirements.txt"])
    {
        add_tool(&mut tools, "Python Linters");
    }
    if any_file(project, &[".golangci.yml", "golangci.yml"]) {
        add_tool(&mut tools, "golangci-lint");
    }
    if any_file(project, &["rustfmt.toml", ".rustfmt.toml"]) {
        add_tool(&mut tools, "rustfmt");
    }
    if any_file(project, &["clippy.toml", ".clippy.toml"]) {
        add_tool(&mut tools, "Clippy");
    }

    if any_file(
        project,
        &["Dockerfile", "docker-compose.yml", "docker-compose.yaml"],
    ) {
        add_tool(&mut tools, "Docker");
    }
    if dir_exists(&p("kubernetes"))
        || (any_file_matches(project, "*.yaml")
            && file_contains(project, "apiVersion:", &["*.yaml"]))
    {
        add_tool(&mut tools, "Kubernetes");
    }

    if tools.is_empty() {
        tools.push("Manual".to_owned());
    }
    tools
}

/// Build systems the project's files point at.
#[must_use]
pub fn detect_build_systems(project: &Path) -> Vec<String> {
    let mut systems = Vec::new();
    let p = |name: &str| project.join(name);

    if file_exists(&p("package.json")) {
        if file_contains(project, "webpack", &["package.json"])
            || any_file(project, &["webpack.config.js", "webpack.config.ts"])
        {
            add_tool(&mut systems, "Webpack");
        }
        if file_contains(project, "vite", &["package.json"])
            || any_file(project, &["vite.config.js", "vite.config.ts"])
        {
            add_tool(&mut systems, "Vite");
        }
        if file_contains(project, "rollup", &["package.json"])
            || any_file(project, &["rollup.config.js", "rollup.config.ts"])
        {
            add_tool(&mut systems, "Rollup");
        }
        if file_contains(project, "parcel", &["package.json"])
            || any_file(project, &["parcelrc", ".parcelrc"])
        {
            add_tool(&mut systems, "Parcel");
        }
        if file_contains(project, "esbuild", &["package.json"])
            || any_file(project, &["esbuild.js", "esbuild.config.js"])
        {
            add_tool(&mut systems, "esbuild");
        }
        if file_contains(project, "turbo", &["package.json"]) || any_file(project, &["turbo.json"])
        {
            add_tool(&mut systems, "Turbopack");
        }
        if file_contains(project, "next", &["package.json"]) {
            add_tool(&mut systems, "Next.js");
        }
        if any_file(project, &["public/index.html"])
            && file_contains(project, "react-scripts", &["package.json"])
        {
            add_tool(&mut systems, "Create React App");
        }
    }

    if file_exists(&p("pyproject.toml")) {
        if file_contains(project, "setuptools", &["pyproject.toml"]) {
            add_tool(&mut systems, "setuptools");
        }
        if file_contains(project, "poetry", &["pyproject.toml"]) {
            add_tool(&mut systems, "Poetry");
        }
        if file_contains(project, "hatch", &["pyproject.toml"]) {
            add_tool(&mut systems, "Hatch");
        }
        if file_contains(project, "pdm", &["pyproject.toml"]) {
            add_tool(&mut systems, "PDM");
        }
    }
    if any_file(project, &["setup.py"]) {
        add_tool(&mut systems, "setuptools");
    }
    if any_file(project, &["Makefile"]) && file_contains(project, "python", &["Makefile"]) {
        add_tool(&mut systems, "Make");
    }

    if any_file(project, &["pom.xml"]) {
        add_tool(&mut systems, "Maven");
    }
    if any_file(project, &["build.gradle", "build.gradle.kts"]) {
        add_tool(&mut systems, "Gradle");
    }
    if any_file(project, &["build.xml"]) {
        add_tool(&mut systems, "Ant");
    }

    if any_file(project, &["go.mod"]) {
        add_tool(&mut systems, "Go Modules");
    }
    if any_file(project, &["Makefile"]) && file_contains(project, "go", &["Makefile"]) {
        add_tool(&mut systems, "Make");
    }

    if any_file(project, &["Cargo.toml"]) {
        add_tool(&mut systems, "Cargo");
    }

    if any_file(project, &["CMakeLists.txt"]) {
        add_tool(&mut systems, "CMake");
    }
    if any_file(project, &["Makefile"]) {
        add_tool(&mut systems, "Make");
    }
    if any_file(project, &["meson.build"]) {
        add_tool(&mut systems, "Meson");
    }
    if any_file(project, &["configure.ac", "Makefile.am"]) {
        add_tool(&mut systems, "Autotools");
    }

    if any_file(project, &["Gemfile"]) {
        add_tool(&mut systems, "Bundler");
    }
    if any_file(project, &["Rakefile"]) {
        add_tool(&mut systems, "Rake");
    }

    if any_file(project, &["composer.json"]) {
        add_tool(&mut systems, "Composer");
    }

    if any_file(project, &["pubspec.yaml"]) {
        add_tool(&mut systems, "Pub");
    }
    if dir_exists(&p("android")) && any_file(project, &["android/build.gradle"]) {
        add_tool(&mut systems, "Gradle (Android)");
    }
    if dir_exists(&p("ios")) && any_file(project, &["ios/Runner.xcodeproj/project.pbxproj"]) {
        add_tool(&mut systems, "Xcode");
    }

    if any_file(project, &["Dockerfile"]) {
        add_tool(&mut systems, "Docker");
    }
    if any_file(project, &["docker-compose.yml", "docker-compose.yaml"]) {
        add_tool(&mut systems, "Docker Compose");
    }
    if any_file(project, &["Terrafile", "main.tf"]) {
        add_tool(&mut systems, "Terraform");
    }

    if systems.is_empty() {
        systems.push("Manual".to_owned());
    }
    systems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str) -> FrameworkHit {
        FrameworkHit {
            framework: name.to_owned(),
            confidence: "high".to_owned(),
        }
    }

    #[test]
    fn the_primary_type_follows_the_priority_list() {
        let hits = vec![hit("go"), hit("react")];
        assert_eq!(detect_project_type(&hits).framework, "react");
        assert_eq!(detect_project_type(&[]).framework, "generic");
    }

    #[test]
    fn languages_default_to_javascript() {
        assert_eq!(
            detect_all_languages(&["README.txt".to_owned()]),
            vec!["JavaScript".to_owned()]
        );
        assert_eq!(
            detect_all_languages(&["a/main.rs".to_owned()]),
            vec!["Rust".to_owned()]
        );
    }
}
