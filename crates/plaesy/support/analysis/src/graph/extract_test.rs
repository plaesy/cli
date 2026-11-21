//! Unit tests for `tests` split out of `extract.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
