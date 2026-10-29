//! Per-file symbols: function, class, and type names, read line by line per language.
//! A heuristic, not a parser. Port of `plaesy-graph-symbols.awk` and the `$SymbolPatterns` table.

use std::sync::OnceLock;

use regex::Regex;

macro_rules! re {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).expect("static regex"))
        }
    };
}

re!(ps1_func, r"^[ \t]*function[ \t]+[A-Za-z0-9_-]+");
re!(ps1_func_strip, r"^[ \t]*function[ \t]+");
re!(sh_func, r"^[ \t]*function[ \t]+[A-Za-z0-9_]+");
re!(sh_func_strip, r"^[ \t]*function[ \t]+");
re!(sh_brace, r"^[ \t]*[A-Za-z0-9_]+[ \t]*\(\)[ \t]*\{");
re!(sh_brace_strip, r"[ \t]*\(\).*");
re!(
    js_func,
    r"^[ \t]*(export[ \t]+)?function[ \t]+[A-Za-z0-9_$]+[ \t]*\("
);
re!(js_func_strip, r".*function[ \t]+");
re!(js_class, r"^[ \t]*(export[ \t]+)?class[ \t]+[A-Za-z0-9_$]+");
re!(js_class_strip, r".*class[ \t]+");
re!(py_def, r"^[ \t]*def[ \t]+[A-Za-z0-9_]+[ \t]*\(");
re!(py_def_strip, r".*def[ \t]+");
re!(py_class, r"^[ \t]*class[ \t]+[A-Za-z0-9_]+");
re!(py_class_strip, r".*class[ \t]+");
re!(
    go_func,
    r"^func[ \t]+(\([^)]*\)[ \t]*)?[A-Za-z0-9_]+[ \t]*\("
);
re!(go_recv_strip, r"^\([^)]*\)[ \t]*");
re!(md_heading, r"^#{1,3}[ \t]+.+$");
re!(
    rust_func,
    r"^[ \t]*(pub(\([^)]*\))?[ \t]+)?(async[ \t]+)?(unsafe[ \t]+)?fn[ \t]+[A-Za-z0-9_]+"
);
re!(rust_func_strip, r".*[ \t]fn[ \t]+|^fn[ \t]+");
re!(
    rust_type,
    r"^[ \t]*(pub(\([^)]*\))?[ \t]+)?(struct|enum|trait)[ \t]+[A-Za-z0-9_]+"
);
re!(rust_type_strip, r"^.*(struct|enum|trait)[ \t]+");
re!(
    dart_decl,
    r"^[ \t]*(?:(?:abstract|base|final|sealed|interface|mixin)[ \t]+)*(?:class|mixin|enum|typedef|extension(?:[ \t]+type)?)[ \t]+([A-Za-z_][A-Za-z0-9_]*)"
);
re!(
    java_class,
    r"^[ \t]*(?:(?:public|private|protected|static|final|abstract)[ \t]+)*(class|interface|enum)[ \t]+[A-Za-z0-9_]+"
);
re!(java_class_strip, r"^.*(class|interface|enum)[ \t]+");
re!(
    kotlin_fun,
    r"^[ \t]*(?:(?:public|private|internal|protected|suspend|inline|override)[ \t]+)*fun[ \t]+[A-Za-z0-9_]+"
);
re!(kotlin_fun_strip, r".*fun[ \t]+");
re!(
    kotlin_type,
    r"^[ \t]*(?:(?:public|private|internal|protected|abstract|open|sealed|data)[ \t]+)*(class|interface|object)[ \t]+[A-Za-z0-9_]+"
);
re!(kotlin_type_strip, r"^.*(class|interface|object)[ \t]+");
re!(
    csharp_type,
    r"^[ \t]*(?:(?:public|private|protected|internal|static|abstract|sealed|partial)[ \t]+)*(class|interface|struct|enum)[ \t]+[A-Za-z0-9_]+"
);
re!(csharp_type_strip, r"^.*(class|interface|struct|enum)[ \t]+");
re!(c_type, r"^[ \t]*(class|struct)[ \t]+[A-Za-z0-9_]+");
re!(c_type_strip, r"^.*(class|struct)[ \t]+");
re!(
    swift_func,
    r"^[ \t]*(?:(?:public|private|internal|fileprivate|open|static|override|mutating)[ \t]+)*func[ \t]+[A-Za-z0-9_]+"
);
re!(swift_func_strip, r".*func[ \t]+");
re!(
    swift_type,
    r"^[ \t]*(?:(?:public|private|internal|fileprivate|open|final)[ \t]+)*(class|struct|enum|protocol)[ \t]+[A-Za-z0-9_]+"
);
re!(swift_type_strip, r"^.*(class|struct|enum|protocol)[ \t]+");
re!(ruby_def, r"^[ \t]*def[ \t]+(self\.)?[A-Za-z0-9_?!=]+");
re!(ruby_def_strip, r".*def[ \t]+");
re!(ruby_type, r"^[ \t]*(class|module)[ \t]+[A-Za-z0-9_:]+");
re!(ruby_type_strip, r"^.*(class|module)[ \t]+");
re!(
    php_func,
    r"^[ \t]*(?:(?:public|private|protected|static)[ \t]+)*function[ \t]+[A-Za-z0-9_]+"
);
re!(php_func_strip, r".*function[ \t]+");
re!(
    php_type,
    r"^[ \t]*(?:(?:abstract|final)[ \t]+)*(class|interface|trait)[ \t]+[A-Za-z0-9_]+"
);
re!(php_type_strip, r"^.*(class|interface|trait)[ \t]+");
re!(
    c_like_func,
    r"^[ \t]*(?:[A-Za-z_][A-Za-z0-9_]*(?:<[^()]*>)?(?:\[\])?[ \t]+)+([A-Za-z_][A-Za-z0-9_]*)[ \t]*\([^;{}]*\)[ \t]*(?:[A-Za-z_][A-Za-z0-9_]*[ \t]*)*\{[ \t]*$"
);
re!(
    c_like_func_no_brace,
    r"^[ \t]*(?:[A-Za-z_][A-Za-z0-9_]*(?:<[^()]*>)?(?:\[\])?[ \t]+)+([A-Za-z_][A-Za-z0-9_]*)[ \t]*\([^;{}]*\)[ \t]*(?:[A-Za-z_][A-Za-z0-9_]*[ \t]*)*$"
);

/// Symbols of a file, chosen by its extension. An unknown extension has none.
#[must_use]
pub fn symbols_of(rel: &str, content: &str) -> Vec<String> {
    let ext = rel
        .rsplit_once('.')
        .map(|(_, e)| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();
    let lines: Vec<&str> = content.split('\n').collect();
    match ext.as_str() {
        ".ps1" => first_match_each(&lines, ps1_func, |m| {
            ps1_func_strip().replace_all(m, "").trim().to_owned()
        }),
        ".sh" => sh_symbols(&lines),
        ".js" | ".jsx" | ".ts" | ".tsx" => js_symbols(&lines),
        ".py" => py_symbols(&lines),
        ".go" => go_symbols(&lines),
        ".md" => md_symbols(&lines),
        ".rs" => rust_symbols(&lines),
        ".dart" => (0..lines.len())
            .flat_map(|i| dart_line_symbols(&lines, i))
            .collect(),
        ".java" => java_symbols(&lines),
        ".kt" | ".kts" => kotlin_symbols(&lines),
        ".cs" => csharp_symbols(&lines),
        ".c" | ".h" | ".cpp" | ".hpp" | ".cc" | ".hh" | ".cxx" => c_family_symbols(&lines),
        ".swift" => swift_symbols(&lines),
        ".rb" => ruby_symbols(&lines),
        ".php" => php_symbols(&lines),
        _ => Vec::new(),
    }
}

/// Applies `pattern` to each line and maps the first match through `extract`.
fn first_match_each(
    lines: &[&str],
    pattern: fn() -> &'static Regex,
    extract: impl Fn(&str) -> String,
) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| pattern().find(line).map(|m| extract(m.as_str())))
        .collect()
}

fn sh_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = sh_func().find(line) {
            out.push(
                sh_func_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        if let Some(m) = sh_brace().find(line) {
            let s = m.as_str().trim_start_matches([' ', '\t']);
            out.push(sh_brace_strip().replace_all(s, "").into_owned());
        }
    }
    out
}

fn js_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = js_func().find(line) {
            let s = m.as_str().trim_end_matches('(');
            out.push(js_func_strip().replace_all(s, "").into_owned());
            continue;
        }
        if let Some(m) = js_class().find(line) {
            out.push(js_class_strip().replace_all(m.as_str(), "").into_owned());
        }
    }
    out
}

fn py_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = py_def().find(line) {
            let s = m.as_str().trim_end_matches('(');
            out.push(py_def_strip().replace_all(s, "").into_owned());
            continue;
        }
        if let Some(m) = py_class().find(line) {
            out.push(py_class_strip().replace_all(m.as_str(), "").into_owned());
        }
    }
    out
}

fn go_symbols(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| go_func().find(line))
        .map(|m| {
            let s = m.as_str().trim_end_matches('(');
            let s = s.trim_start_matches("func").trim_start_matches([' ', '\t']);
            go_recv_strip().replace_all(s, "").into_owned()
        })
        .collect()
}

fn md_symbols(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| md_heading().is_match(line))
        .map(|line| {
            let hashes = line.bytes().take_while(|&b| b == b'#').count();
            line[hashes..]
                .trim_start_matches([' ', '\t'])
                .trim_end_matches([' ', '\t'])
                .to_owned()
        })
        .collect()
}

fn rust_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = rust_func().find(line) {
            out.push(
                rust_func_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        if let Some(m) = rust_type().find(line) {
            out.push(
                rust_type_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
        }
    }
    out
}

/// The symbol declared on `lines[i]` of a Dart file: a type name, else a method or function name.
/// A type declaration wins because `extension type Foo(int x) {` also reads as a C-like method.
#[must_use]
pub fn dart_line_symbols(lines: &[&str], i: usize) -> Vec<String> {
    if let Some(caps) = dart_decl().captures(lines[i]) {
        return vec![caps[1].to_owned()];
    }
    match c_like_func_name_at(lines, i) {
        Some(name) => vec![name],
        None => Vec::new(),
    }
}

/// The method name declared on `lines[i]` when it is a C-like declaration. The same-line `{`
/// form and the Allman form (brace alone on the next non-blank line) both count.
#[must_use]
pub fn c_like_func_name_at(lines: &[&str], i: usize) -> Option<String> {
    if let Some(caps) = c_like_func().captures(lines[i]) {
        return Some(caps[1].to_owned());
    }
    let caps = c_like_func_no_brace().captures(lines[i])?;
    let next = (i + 1..lines.len()).find(|&j| !lines[j].trim().is_empty())?;
    (lines[next].trim() == "{").then(|| caps[1].to_owned())
}

/// Type hits plus C-like method hits, restored to source order.
fn merge_with_methods(type_hits: Vec<(usize, String)>, lines: &[&str]) -> Vec<String> {
    let mut hits = type_hits;
    for (i, _) in lines.iter().enumerate() {
        if let Some(name) = c_like_func_name_at(lines, i) {
            hits.push((i, name));
        }
    }
    hits.sort_by_key(|(line, _)| *line);
    hits.into_iter().map(|(_, name)| name).collect()
}

fn java_symbols(lines: &[&str]) -> Vec<String> {
    let hits = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            java_class().find(line).map(|m| {
                (
                    i,
                    java_class_strip()
                        .replace_all(m.as_str(), "")
                        .trim()
                        .to_owned(),
                )
            })
        })
        .collect();
    merge_with_methods(hits, lines)
}

fn kotlin_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = kotlin_fun().find(line) {
            out.push(
                kotlin_fun_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        if let Some(m) = kotlin_type().find(line) {
            out.push(
                kotlin_type_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
        }
    }
    out
}

fn csharp_symbols(lines: &[&str]) -> Vec<String> {
    let hits = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            csharp_type().find(line).map(|m| {
                (
                    i,
                    csharp_type_strip()
                        .replace_all(m.as_str(), "")
                        .trim()
                        .to_owned(),
                )
            })
        })
        .collect();
    merge_with_methods(hits, lines)
}

fn c_family_symbols(lines: &[&str]) -> Vec<String> {
    let hits = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| {
            c_type().find(line).map(|m| {
                (
                    i,
                    c_type_strip().replace_all(m.as_str(), "").trim().to_owned(),
                )
            })
        })
        .collect();
    merge_with_methods(hits, lines)
}

fn swift_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = swift_func().find(line) {
            out.push(
                swift_func_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        if let Some(m) = swift_type().find(line) {
            out.push(
                swift_type_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
        }
    }
    out
}

fn ruby_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = ruby_def().find(line) {
            out.push(
                ruby_def_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        if let Some(m) = ruby_type().find(line) {
            out.push(
                ruby_type_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
        }
    }
    out
}

fn php_symbols(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if let Some(m) = php_func().find(line) {
            out.push(
                php_func_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        if let Some(m) = php_type().find(line) {
            out.push(
                php_type_strip()
                    .replace_all(m.as_str(), "")
                    .trim()
                    .to_owned(),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_functions_and_methods_are_named() {
        let src = "func Build(x int) int {\nfunc (g *Graph) Save() error {\n";
        assert_eq!(
            symbols_of("a.go", src),
            ["Build", "Save"].map(String::from).to_vec()
        );
    }

    #[test]
    fn markdown_headings_are_listed_without_their_marks() {
        assert_eq!(
            symbols_of("a.md", "# Title\n## Part\n#### Deep"),
            ["Title", "Part"].map(String::from).to_vec()
        );
    }

    #[test]
    fn dart_type_wins_over_a_method_shape() {
        let lines = ["extension type Foo(int x) {", "void run() {"];
        assert_eq!(dart_line_symbols(&lines, 0), ["Foo"]);
        assert_eq!(dart_line_symbols(&lines, 1), ["run"]);
    }

    #[test]
    fn unknown_extension_has_no_symbols() {
        assert!(symbols_of("a.bin", "class X").is_empty());
    }
}
