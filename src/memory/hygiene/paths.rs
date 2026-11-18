//! `paths` helpers split out of `mod.rs`.

use super::*;

/// File extensions treated as "this backticked token is probably a file path".
const PATH_EXTENSIONS: [&str; 12] = [
    "md", "rs", "toml", "json", "yaml", "yml", "py", "sh", "ts", "js", "txt", "sql",
];

/// Backticked tokens in `body` that look like relative file paths, with their 1-based line.
pub fn path_references(body: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in body.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for (n, part) in line.split('`').enumerate() {
            if n % 2 == 0 || part.is_empty() {
                continue; // outside backticks
            }
            let tok = part.trim();
            let ext_ok = tok
                .rsplit_once('.')
                .is_some_and(|(_, e)| PATH_EXTENSIONS.contains(&e));
            let safe_relative_path = std::path::Path::new(tok)
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)));
            let shape_ok = safe_relative_path
                && !tok.contains(char::is_whitespace)
                && !tok.contains([
                    '*', '<', '>', '{', '}', '(', ')', '$', ':', '|', '?', '=', '[', ']',
                ])
                && !tok.starts_with(['/', '~', '.', '@', '#'])
                && !tok.ends_with('/')
                && tok.len() < 200;
            // Require a directory component: a bare `foo.md` is as likely a name as a path.
            if ext_ok && shape_ok && tok.contains('/') {
                out.push((tok.to_string(), i + 1));
            }
        }
    }
    out
}

/// Resolve a repository-relative path against a mapped checkout.
///
/// Also accepts one leading component when it exactly matches the local checkout directory name,
/// so references like `cli/src/main.rs` resolve when the mapped checkout itself is named `cli`.
pub(super) fn exists_in_source_root(source_root: &std::path::Path, token: &str) -> bool {
    let token_path = std::path::Path::new(token);
    if source_root.join(token_path).exists() {
        return true;
    }

    let components: Vec<_> = token_path.components().collect();
    if components
        .iter()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return false;
    }
    let Some(std::path::Component::Normal(prefix)) = components.first() else {
        return false;
    };
    let Some(root_name) = source_root.file_name() else {
        return false;
    };
    if *prefix != root_name || components.len() < 2 {
        return false;
    }

    let suffix: PathBuf = components[1..].iter().collect();
    source_root.join(suffix).exists()
}
