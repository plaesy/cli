//! Structural extraction: the references a file makes to other nodes.
//!
//! Each pass resolves a reference against the known node ids. A reference that does not
//! resolve to a node gives no edge.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::exclude::Ignore;

use super::scan::{normalize_path, read_all, resolve_rel_link};
use super::types::Edge;

macro_rules! re {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).expect("static regex"))
        }
    };
}

re!(md_link, r"\[[^\]]*\]\(([^)\s]+)\)");
re!(ps1_dot_source, r#"(?m)(?:^|\s)\.\s+["']?([^"'\r\n]+\.ps1)"#);
re!(ps1_call, r#"&\s+["']([^"']+\.ps1)"#);
re!(
    sh_source,
    r#"(?m)(?:^|\s)(?:source|\.)\s+["']?([^"'\r\n]+\.sh)"#
);
re!(
    js_import,
    r#"import\s+.*?from\s+['"]([^'"]+)['"]|require\(['"]([^'"]+)['"]\)"#
);
re!(
    py_import,
    r"^\s*(?:from\s+([\w.]+)(?:\s+import)|import\s+([\w.]+))"
);
re!(go_import_line, r#"^\s*(?:import\s+)?"([^"]+)"\s*$"#);
re!(go_module_line, r"(?m)^module\s+(\S+)");
re!(
    dart_import,
    r#"(?m)^\s*(?:import|export|part)\s+['"]([^'"]+)['"]"#
);
re!(pubspec_name, r"(?m)^name:\s*(\S+)");
re!(java_import, r"^\s*import\s+(?:static\s+)?([\w.]+)\s*;");

/// Java source roots. The rightmost match in a path marks where the package path starts.
const JAVA_SOURCE_ROOTS: [&str; 12] = [
    "src/main/java/",
    "/src/main/java/",
    "src/test/java/",
    "/src/test/java/",
    "src/main/kotlin/",
    "/src/main/kotlin/",
    "src/test/kotlin/",
    "/src/test/kotlin/",
    "src/",
    "/src/",
    "java/",
    "/java/",
];

/// Indexes built once per run, so one file's edges can be computed on their own.
#[derive(Debug, Default)]
pub struct ExtractContext {
    pub repo_root_slash: String,
    pub known: BTreeSet<String>,
    /// Module directory (`""` for the root) to its declared Go module path.
    pub gomod_path: BTreeMap<String, String>,
    /// Directory to the non-test `.go` files in it.
    pub go_dir_files: BTreeMap<String, Vec<String>>,
    /// Dart package name to its `lib` directory.
    pub dart_pkg_lib: BTreeMap<String, String>,
    /// Dotted Java class name to its file.
    pub java_index: BTreeMap<String, String>,
}

impl ExtractContext {
    #[must_use]
    pub fn new(repo_root_slash: &str, files: &[String], known: &BTreeSet<String>) -> Self {
        Self {
            repo_root_slash: repo_root_slash.to_owned(),
            known: known.clone(),
            gomod_path: scan_go_modules(repo_root_slash),
            go_dir_files: index_go_dir_files(files),
            dart_pkg_lib: scan_dart_pubspecs(repo_root_slash),
            java_index: index_java_files(files),
        }
    }

    /// The structural edges that start at `rel`, whose content is `content`.
    #[must_use]
    pub fn file_edges(&self, rel: &str, content: &str) -> Vec<Edge> {
        if content.is_empty() {
            return Vec::new();
        }
        let root = self.repo_root_slash.as_str();
        let mut edges = Vec::new();
        let mut add = |target: Option<String>, kind: &str| {
            if let Some(tgt) = target.filter(|t| !t.is_empty() && t != rel) {
                edges.push(Edge {
                    source: rel.to_owned(),
                    target: tgt,
                    kind: kind.to_owned(),
                    confidence: "EXTRACTED".into(),
                });
            }
        };
        match extension(rel).as_str() {
            ".md" => {
                for caps in md_link().captures_iter(content) {
                    add(
                        resolve_rel_link(root, rel, &caps[1], &self.known),
                        "references",
                    );
                }
            }
            ".ps1" => {
                for caps in ps1_dot_source().captures_iter(content) {
                    add(
                        resolve_rel_link(root, rel, &clean_ps1_target(&caps[1]), &self.known),
                        "sources",
                    );
                }
                for caps in ps1_call().captures_iter(content) {
                    add(
                        resolve_rel_link(root, rel, &clean_ps1_target(&caps[1]), &self.known),
                        "calls",
                    );
                }
            }
            ".sh" => {
                for caps in sh_source().captures_iter(content) {
                    add(
                        resolve_rel_link(root, rel, &caps[1], &self.known),
                        "sources",
                    );
                }
            }
            ".js" | ".jsx" | ".ts" | ".tsx" => {
                for caps in js_import().captures_iter(content) {
                    let spec = first_group(&caps);
                    if spec.starts_with('.') {
                        add(self.resolve_js_import(rel, &spec), "imports");
                    }
                }
            }
            ".py" => {
                for line in content.split('\n') {
                    if let Some(caps) = py_import().captures(line) {
                        add(self.resolve_py_import(rel, &first_group(&caps)), "imports");
                    }
                }
            }
            ".go" => {
                if !self.gomod_path.is_empty() {
                    for line in content.split('\n') {
                        if let Some(caps) = go_import_line().captures(line) {
                            for target in self.resolve_go_import(rel, &caps[1]) {
                                add(Some(target), "imports");
                            }
                        }
                    }
                }
            }
            ".dart" => {
                for caps in dart_import().captures_iter(content) {
                    let spec = caps[1].to_owned();
                    if !spec.starts_with("dart:") {
                        add(self.resolve_dart_import(rel, &spec), "imports");
                    }
                }
            }
            ".java" => {
                for line in content.split('\n') {
                    if let Some(caps) = java_import().captures(line) {
                        let spec = &caps[1];
                        if !spec.ends_with('*') {
                            add(self.java_index.get(spec).cloned(), "imports");
                        }
                    }
                }
            }
            _ => {}
        }
        edges
    }

    fn resolve_js_import(&self, rel: &str, spec: &str) -> Option<String> {
        let from_dir = from_dir(&self.repo_root_slash, rel);
        let candidates = [
            spec.to_owned(),
            format!("{spec}.js"),
            format!("{spec}.jsx"),
            format!("{spec}.ts"),
            format!("{spec}.tsx"),
            format!("{spec}/index.js"),
            format!("{spec}/index.ts"),
        ];
        let prefix = format!("{}/", self.repo_root_slash);
        candidates.iter().find_map(|cand| {
            let resolved = normalize_path(&from_dir, cand);
            if !format!("{resolved}/").starts_with(&prefix) {
                return None;
            }
            let rel_c = &resolved[prefix.len()..];
            self.known.contains(rel_c).then(|| rel_c.to_owned())
        })
    }

    fn resolve_py_import(&self, rel: &str, spec: &str) -> Option<String> {
        if spec.is_empty() || spec.chars().all(|c| c == '.') {
            return None;
        }
        let spec_path = spec.replace('.', "/");
        let mut from_dirs = vec![self.repo_root_slash.clone()];
        if rel.contains('/') {
            from_dirs.insert(0, from_dir(&self.repo_root_slash, rel));
        }
        let prefix = format!("{}/", self.repo_root_slash);
        from_dirs.iter().find_map(|dir| {
            let resolved = normalize_path(dir, &format!("{spec_path}.py"));
            if !format!("{resolved}/").starts_with(&prefix) {
                return None;
            }
            let rel_c = &resolved[prefix.len()..];
            self.known.contains(rel_c).then(|| rel_c.to_owned())
        })
    }

    /// Files a Go import reaches: the package directory, found from the longest module prefix.
    fn resolve_go_import(&self, rel: &str, spec: &str) -> Vec<String> {
        let rel_dir = rel.rfind('/').map_or("", |i| &rel[..i]);
        let mut best: Option<(&str, usize)> = None;
        for gd in self.gomod_path.keys() {
            let matches = gd.is_empty() || rel_dir == gd || rel_dir.starts_with(&format!("{gd}/"));
            if matches && best.is_none_or(|(_, len)| gd.len() > len) {
                best = Some((gd.as_str(), gd.len()));
            }
        }
        let Some((best_dir, _)) = best else {
            return Vec::new();
        };
        let modpath = &self.gomod_path[best_dir];
        if spec != modpath && !spec.starts_with(&format!("{modpath}/")) {
            return Vec::new();
        }
        let subpath = spec[modpath.len()..].trim_start_matches('/');
        let target_dir = match (best_dir.is_empty(), subpath.is_empty()) {
            (_, true) => best_dir.to_owned(),
            (true, false) => subpath.to_owned(),
            (false, false) => format!("{best_dir}/{subpath}"),
        };
        self.go_dir_files
            .get(&target_dir)
            .map(|files| {
                files
                    .iter()
                    .filter(|f| f.as_str() != rel)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn resolve_dart_import(&self, rel: &str, spec: &str) -> Option<String> {
        let resolved_full = if let Some(pkgspec) = spec.strip_prefix("package:") {
            let (pkgname, pkgpath) = pkgspec.split_once('/').unwrap_or((pkgspec, ""));
            let libdir = self.dart_pkg_lib.get(pkgname)?;
            normalize_path(&format!("{}/{libdir}", self.repo_root_slash), pkgpath)
        } else {
            normalize_path(&from_dir(&self.repo_root_slash, rel), spec)
        };
        let prefix = format!("{}/", self.repo_root_slash);
        if !format!("{resolved_full}/").starts_with(&prefix) {
            return None;
        }
        let rel_c = &resolved_full[prefix.len()..];
        self.known.contains(rel_c).then(|| rel_c.to_owned())
    }
}

fn extension(rel: &str) -> String {
    rel.rfind('.')
        .map(|i| rel[i..].to_lowercase())
        .unwrap_or_default()
}

fn from_dir(root: &str, rel: &str) -> String {
    match rel.rfind('/') {
        Some(i) => format!("{root}/{}", &rel[..i]),
        None => root.to_owned(),
    }
}

fn first_group(caps: &regex::Captures<'_>) -> String {
    (1..caps.len())
        .find_map(|i| caps.get(i).map(|m| m.as_str().to_owned()))
        .unwrap_or_default()
}

fn clean_ps1_target(target: &str) -> String {
    let t = target
        .strip_prefix("$PSScriptRoot\\")
        .or_else(|| target.strip_prefix("$PSScriptRoot/"))
        .unwrap_or(target);
    t.replace('\\', "/")
}

/// Walks the repo for files named `name`, honouring the same exclusions as the source scan.
pub fn find_files_named(root: &str, name: &str) -> Vec<String> {
    let root_path = Path::new(root);
    let ignore = Ignore::load(root_path);
    let mut out = Vec::new();
    find_walk(root_path, root_path, name, &ignore, &mut out);
    out.sort();
    out
}

fn find_walk(root: &Path, dir: &Path, name: &str, ignore: &Ignore, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let base = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !ignore.skip_dir(&rel, &base) {
                find_walk(root, &path, name, ignore, out);
            }
        } else if base == name && !ignore.skip_file(&rel) {
            out.push(rel);
        }
    }
}

/// Module directory to its declared module path, for every `go.mod` in the tree.
fn scan_go_modules(root: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for rel in find_files_named(root, "go.mod") {
        let content = read_all(&Path::new(root).join(&rel));
        let Some(caps) = go_module_line().captures(&content) else {
            continue;
        };
        let dir = rel
            .strip_suffix("/go.mod")
            .map_or(String::new(), str::to_owned);
        out.insert(dir, caps[1].to_owned());
    }
    out
}

fn index_go_dir_files(files: &[String]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rel in files {
        if !rel.ends_with(".go") || rel.ends_with("_test.go") {
            continue;
        }
        let dir = rel
            .rfind('/')
            .map_or(String::new(), |i| rel[..i].to_owned());
        out.entry(dir).or_default().push(rel.clone());
    }
    out
}

/// Package name to its `lib` directory, for every `pubspec.yaml` in the tree.
fn scan_dart_pubspecs(root: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for rel in find_files_named(root, "pubspec.yaml") {
        let content = read_all(&Path::new(root).join(&rel));
        let Some(caps) = pubspec_name().captures(&content) else {
            continue;
        };
        let dir = rel
            .strip_suffix("/pubspec.yaml")
            .map_or(".".to_owned(), str::to_owned);
        out.insert(caps[1].to_owned(), format!("{dir}/lib"));
    }
    out
}

/// Dotted class name to file, for Java files under a recognised source root.
fn index_java_files(files: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for rel in files.iter().filter(|r| r.ends_with(".java")) {
        let cut = JAVA_SOURCE_ROOTS
            .iter()
            .filter_map(|root| rel.rfind(root).map(|i| i + root.len()))
            .max();
        let Some(cut) = cut else {
            continue;
        };
        let dotted = rel[cut..].trim_end_matches(".java").replace('/', ".");
        out.insert(dotted, rel.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn markdown_links_become_references_edges() {
        let k = known(&["docs/b.md"]);
        let files = vec!["docs/a.md".to_owned(), "docs/b.md".to_owned()];
        let ctx = ExtractContext::new("/repo", &files, &k);
        let edges = ctx.file_edges("docs/a.md", "see [b](b.md)");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].kind, "references");
        assert_eq!(edges[0].target, "docs/b.md");
    }

    #[test]
    fn js_relative_imports_resolve_to_a_known_file() {
        let k = known(&["src/util.ts"]);
        let files = vec!["src/app.ts".to_owned(), "src/util.ts".to_owned()];
        let ctx = ExtractContext::new("/repo", &files, &k);
        let edges = ctx.file_edges("src/app.ts", "import x from './util';");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, "src/util.ts");
    }

    #[test]
    fn java_class_names_come_from_the_source_root() {
        let files = vec!["maven/src/main/java/com/example/Helper.java".to_owned()];
        let index = index_java_files(&files);
        assert_eq!(
            index["com.example.Helper"],
            "maven/src/main/java/com/example/Helper.java"
        );
    }
}
