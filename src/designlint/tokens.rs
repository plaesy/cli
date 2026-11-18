//! `tokens` helpers split out of `designlint.rs`.

use super::*;

/// Colour tokens declared across the run. A token declared with more than one
/// value is a conflict, and it is not resolved.
#[derive(Debug, Default)]
pub(super) struct Tokens {
    colours: std::collections::BTreeMap<String, Colour>,
    pub(super) conflicts: std::collections::BTreeMap<String, usize>,
}

fn token_decl_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"--([A-Za-z0-9_-]+)\s*:\s*([^;}{]+)").expect("static regex"))
}

pub(super) fn collect_tokens(contents: &[String]) -> Tokens {
    let mut seen: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        Default::default();
    let mut tokens = Tokens::default();
    for text in contents {
        for m in token_decl_re().captures_iter(text) {
            let Some(colour) = parse_colour(&m[2]) else {
                continue;
            };
            seen.entry(m[1].to_owned())
                .or_default()
                .insert(colour.key());
            tokens.colours.entry(m[1].to_owned()).or_insert(colour);
        }
    }
    for (name, values) in seen {
        if values.len() > 1 {
            tokens.conflicts.insert(name.clone(), values.len());
            tokens.colours.remove(&name);
        }
    }
    tokens
}

impl Tokens {
    /// The colour a value stands for: a literal, or `var(--name)`. Gradients and images do not resolve.
    pub(super) fn resolve(&self, value: &str) -> Option<Colour> {
        let lower = value.to_lowercase();
        if lower.contains("gradient") || lower.contains("url(") {
            return None;
        }
        if lower.starts_with("var(") {
            let inner = value.trim()[4..].trim().trim_end_matches(')').trim();
            let name = inner.trim_start_matches("--");
            let name = name.split(',').next().unwrap_or("").trim();
            return self.colours.get(name).copied();
        }
        parse_colour(value)
    }
}

/// Splits `s` at `sep` wherever `sep` is outside parentheses and quotes.
pub(super) fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth -= 1,
                _ if c == sep && depth == 0 => {
                    parts.push(&s[start..i]);
                    start = i + c.len_utf8();
                }
                _ => {}
            },
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Reads `prop: value; prop: value` into a map of lower-case property names.
pub(super) fn css_decls(body: &str) -> std::collections::BTreeMap<String, String> {
    split_top(body, ';')
        .into_iter()
        .filter_map(|decl| {
            let (prop, value) = decl.split_once(':')?;
            let prop = prop.trim().to_lowercase();
            (!prop.is_empty()).then(|| (prop, value.trim().to_owned()))
        })
        .collect()
}

/// Reads a React style object body into CSS property names. A bare number for
/// `fontSize` means pixels.
pub(super) fn jsx_decls(body: &str) -> std::collections::BTreeMap<String, String> {
    split_top(body, ',')
        .into_iter()
        .filter_map(|pair| {
            let (key, value) = pair.split_once(':')?;
            let key = camel_to_kebab(key.trim());
            let mut value = value.trim().trim_matches(['\'', '"']).to_owned();
            if key == "font-size" && value.parse::<f64>().is_ok() {
                value.push_str("px");
            }
            (!key.is_empty()).then_some((key, value))
        })
        .collect()
}

fn camel_to_kebab(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
