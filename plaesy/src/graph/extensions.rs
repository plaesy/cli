//! Extensions the graph collects, and which of them count as source code.

/// Extensions treated as source code. Each must also have a symbol extractor, or its files get no symbols.
pub const SOURCE_EXTENSIONS: [&str; 22] = [
    ".js", ".jsx", ".ts", ".tsx", ".py", ".go", ".dart", ".java", ".kt", ".kts", ".swift", ".c",
    ".h", ".cc", ".cpp", ".hpp", ".cxx", ".hh", ".cs", ".rs", ".rb", ".php",
];

/// Collected but not labelled source: Markdown documents and shell scripts.
pub const NON_SOURCE_EXTENSIONS: [&str; 3] = [".md", ".sh", ".ps1"];

/// Reports whether a path's extension is a source extension.
#[must_use]
pub fn is_source_ext(rel: &str) -> bool {
    let ext = rel
        .rsplit_once('.')
        .map(|(_, e)| format!(".{}", e.to_lowercase()));
    ext.is_some_and(|ext| SOURCE_EXTENSIONS.contains(&ext.as_str()))
}

/// Reports whether the graph collects a path at all.
#[must_use]
pub fn is_collected(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    SOURCE_EXTENSIONS
        .iter()
        .chain(NON_SOURCE_EXTENSIONS.iter())
        .any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collected_and_source_are_different_sets() {
        assert!(is_collected("docs/readme.md"));
        assert!(!is_source_ext("docs/readme.md"));
        assert!(is_source_ext("src/Main.RS"));
        assert!(!is_collected("image.png"));
    }
}
