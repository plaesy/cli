//! A Markdown linter that implements the markdownlint rule set directly, with the
//! same rule IDs and config keys, plus the three GitHub accessibility rules.
//!
//! Two behaviours are deliberate. Unknown rule names and unknown option keys are
//! errors, never silent no-ops. `extends` is rejected, because there is no registry
//! to resolve it against.
//! Port of `scripts/internal/mdlint` (Go). The parser is `pulldown-cmark` in place
//! of goldmark. Rule coverage is a subset of markdownlint's rules.

pub mod ast_rules;
pub mod doc;
pub mod fix;
pub mod github;
pub mod text_rules;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use doc::Doc;

/// The config file read from the tree root when none is passed.
pub const DEFAULT_CONFIG_NAME: &str = ".markdownlint.json";
/// The ratchet file the CLI reads by default.
pub const DEFAULT_BASELINE_NAME: &str = ".markdownlint-baseline.json";
/// Directories never walked. The per-platform command folders are generated copies,
/// so they are excluded to keep the total independent of local state.
pub const DEFAULT_IGNORE_DIRS: [&str; 10] = [
    ".git",
    "node_modules",
    ".plaesy",
    "dist",
    "vendor",
    "bin",
    ".idea",
    ".vscode",
    ".claude",
    ".kilo",
];

/// Errors from linting.
#[derive(Debug, Error)]
pub enum MdError {
    #[error("config: {0}")]
    Config(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
}

/// One reported problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: &'static str,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Rule check: the violations of one document under a compiled rule set.
pub type Check = fn(&Doc, &RuleSet) -> Vec<Violation>;

/// Validates a rule's options object.
pub type Validate = fn(&Value) -> Result<(), String>;

/// One implemented rule.
#[derive(Clone)]
pub struct RuleSpec {
    pub id: &'static str,
    pub aliases: &'static [&'static str],
    pub check: Check,
    pub validate: Option<Validate>,
    pub fixable: bool,
}

/// Enabled rules with their option values.
#[derive(Default)]
pub struct RuleSet {
    pub rules: Vec<RuleSpec>,
    pub options: BTreeMap<&'static str, Value>,
}

impl RuleSet {
    /// The options of `id`, or the defaults when the config did not set them.
    #[must_use]
    pub fn opt<T: DeserializeOwned + Default>(&self, id: &str) -> T {
        self.options
            .get(id)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default()
    }
}

fn validator<T: DeserializeOwned>(value: &Value) -> Result<(), String> {
    serde_json::from_value::<T>(value.clone())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

macro_rules! spec {
    ($id:expr, [$($alias:expr),*], $check:path, $validate:expr, $fixable:expr) => {
        RuleSpec {
            id: $id,
            aliases: &[$($alias),*],
            check: $check,
            validate: $validate,
            fixable: $fixable,
        }
    };
}

/// Every implemented rule.
#[must_use]
pub fn all_rules() -> Vec<RuleSpec> {
    use ast_rules as a;
    use text_rules as t;
    vec![
        spec!(
            "MD001",
            ["heading-increment"],
            a::md001,
            Some(validator::<Md001Options>),
            false
        ),
        spec!(
            "MD003",
            ["heading-style"],
            a::md003,
            Some(validator::<a::Md003Options>),
            false
        ),
        spec!(
            "MD004",
            ["ul-style"],
            t::md004,
            Some(validator::<t::Md004Options>),
            false
        ),
        spec!(
            "MD007",
            ["ul-indent"],
            t::md007,
            Some(validator::<t::Md007Options>),
            false
        ),
        spec!(
            "MD009",
            ["no-trailing-spaces"],
            t::md009,
            Some(validator::<t::Md009Options>),
            true
        ),
        spec!(
            "MD010",
            ["no-hard-tabs"],
            t::md010,
            Some(validator::<t::Md010Options>),
            true
        ),
        spec!(
            "MD012",
            ["no-multiple-blanks"],
            t::md012,
            Some(validator::<t::Md012Options>),
            true
        ),
        spec!(
            "MD013",
            ["line-length"],
            t::md013,
            Some(validator::<t::Md013Options>),
            false
        ),
        spec!(
            "MD022",
            ["blanks-around-headings"],
            t::md022,
            Some(validator::<t::Md022Options>),
            true
        ),
        spec!(
            "MD024",
            ["no-duplicate-heading"],
            a::md024,
            Some(validator::<a::Md024Options>),
            false
        ),
        spec!(
            "MD025",
            ["single-title", "single-h1"],
            a::md025,
            Some(validator::<a::Md025Options>),
            false
        ),
        spec!(
            "MD026",
            ["no-trailing-punctuation"],
            t::md026,
            Some(validator::<t::Md026Options>),
            false
        ),
        spec!(
            "MD029",
            ["ol-prefix"],
            t::md029,
            Some(validator::<t::Md029Options>),
            false
        ),
        spec!(
            "MD031",
            ["blanks-around-fences"],
            t::md031,
            Some(validator::<Md031Options>),
            true
        ),
        spec!(
            "MD032",
            ["blanks-around-lists"],
            t::md032,
            Some(validator::<Md032Options>),
            true
        ),
        spec!(
            "MD036",
            ["no-emphasis-as-heading"],
            a::md036,
            Some(validator::<a::Md036Options>),
            false
        ),
        spec!(
            "MD040",
            ["fenced-code-language"],
            t::md040,
            Some(validator::<t::Md040Options>),
            false
        ),
        spec!("MD041", ["first-line-heading"], a::md041, None, false),
        spec!(
            "MD046",
            ["code-block-style"],
            a::md046,
            Some(validator::<a::Md046Options>),
            false
        ),
        spec!(
            "MD047",
            ["single-trailing-newline"],
            t::md047,
            Some(validator::<Md047Options>),
            true
        ),
        spec!(
            "MD048",
            ["code-fence-style"],
            a::md048,
            Some(validator::<a::Md048Options>),
            false
        ),
        spec!(
            "MD058",
            ["blanks-around-tables"],
            t::md058,
            Some(validator::<Md058Options>),
            true
        ),
        spec!("GHA001", ["no-empty-alt-text"], github::gha001, None, false),
        spec!(
            "GHA002",
            ["no-default-alt-text"],
            github::gha002,
            None,
            false
        ),
        spec!(
            "GHA003",
            ["no-generic-link-text"],
            github::gha003,
            None,
            false
        ),
    ]
}

/// Options for rules that have no dedicated implementation module.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md001Options {
    pub heading_levels: Vec<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md031Options {
    pub list_items: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md032Options {
    pub lists: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md047Options {}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Md058Options {
    pub tables: Option<bool>,
}

/// A parsed `.markdownlint.json`.
#[derive(Debug, Clone)]
pub struct Config {
    pub default: bool,
    pub rules: BTreeMap<String, Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default: true,
            rules: BTreeMap::new(),
        }
    }
}

/// Reads a config file. A missing file means the defaults.
///
/// # Errors
///
/// Invalid JSON, a non-boolean `default`, or `extends`.
pub fn load_config(path: &Path) -> Result<Config, MdError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(source) => {
            return Err(MdError::Io {
                context: format!("reading {}", path.display()),
                source,
            })
        }
    };
    let raw: serde_json::Map<String, Value> = serde_json::from_str(&text)
        .map_err(|e| MdError::Config(format!("parse {}: {e}", path.display())))?;
    let mut cfg = Config::default();
    if let Some(value) = raw.get("default") {
        cfg.default = value.as_bool().ok_or_else(|| {
            MdError::Config(format!(
                "parse {}: 'default' must be a boolean",
                path.display()
            ))
        })?;
    }
    if raw.contains_key("extends") {
        return Err(MdError::Config(format!(
            "{} uses \"extends\": npm shareable configs cannot be resolved by the Rust linter — copy the rules you need into the file",
            path.display()
        )));
    }
    for (key, value) in raw {
        if key == "default" || key.starts_with('$') {
            continue;
        }
        cfg.rules.insert(key, value);
    }
    Ok(cfg)
}

/// Compiles a config into the enabled rules with their options. Unknown rule names,
/// unknown option keys, and options given to a rule with none are errors.
///
/// # Errors
///
/// [`MdError::Config`] for any of the above.
pub fn compile(cfg: &Config) -> Result<RuleSet, MdError> {
    let specs = all_rules();
    let mut by_key: BTreeMap<&str, &RuleSpec> = BTreeMap::new();
    for spec in &specs {
        by_key.insert(spec.id, spec);
        for alias in spec.aliases {
            by_key.insert(alias, spec);
        }
    }

    let mut set = RuleSet::default();
    let mut explicit_off: BTreeSet<&'static str> = BTreeSet::new();
    let mut mentioned: BTreeSet<&'static str> = BTreeSet::new();

    for (key, value) in &cfg.rules {
        let Some(spec) = by_key.get(key.as_str()) else {
            // A rule this linter does not implement, switched off, is already off.
            if value.as_bool() == Some(false) {
                continue;
            }
            return Err(MdError::Config(format!(
                "rule {key:?} is not implemented, so enabling or configuring it would silently do nothing — implemented rules are: {}",
                implemented_ids(&specs).join(", ")
            )));
        };
        mentioned.insert(spec.id);
        if let Some(on) = value.as_bool() {
            if !on {
                explicit_off.insert(spec.id);
            }
            continue;
        }
        let Some(validate) = spec.validate else {
            return Err(MdError::Config(format!(
                "rule {:?} takes no options — give it true or false",
                spec.id
            )));
        };
        validate(value).map_err(|e| MdError::Config(format!("rule {:?}: {e}", spec.id)))?;
        set.options.insert(spec.id, value.clone());
    }

    for spec in specs {
        if explicit_off.contains(spec.id) {
            continue;
        }
        if !cfg.default && !mentioned.contains(spec.id) {
            continue;
        }
        set.rules.push(spec);
    }
    set.rules.sort_by(|a, b| a.id.cmp(b.id));
    Ok(set)
}

fn implemented_ids(specs: &[RuleSpec]) -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = specs.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    ids
}

/// Per-file detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResult {
    pub path: String,
    pub total: usize,
    pub detail: Vec<Violation>,
}

/// Outcome of a lint run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LintResult {
    /// Files with at least one violation.
    pub files: usize,
    pub scanned: usize,
    pub total: usize,
    pub by_rule: BTreeMap<String, usize>,
    pub by_file: BTreeMap<String, usize>,
    /// Worst files first.
    pub violations: Vec<FileResult>,
}

impl LintResult {
    /// Per-rule counts, most frequent first, one line per rule.
    #[must_use]
    pub fn histogram(&self) -> String {
        if self.by_rule.is_empty() {
            return "no violations".into();
        }
        let mut pairs: Vec<(&String, &usize)> = self.by_rule.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        pairs
            .iter()
            .map(|(rule, n)| format!("  {rule:<8} {n}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Up to `n` file paths, most violations first.
    #[must_use]
    pub fn worst_files(&self, n: usize) -> Vec<&str> {
        self.violations
            .iter()
            .take(n)
            .map(|f| f.path.as_str())
            .collect()
    }

    /// Whether the run is above the baseline (zero when none is given).
    #[must_use]
    pub fn exceeds(&self, baseline: Option<&Baseline>) -> bool {
        match baseline {
            None => self.total > 0,
            Some(b) => self.total > b.max_violations,
        }
    }
}

/// A ratchet ceiling. It may only go down.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct Baseline {
    pub max_violations: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_rule: BTreeMap<String, usize>,
    #[serde(default)]
    pub measured_at: String,
    #[serde(default)]
    pub note: String,
}

/// Reads a baseline. A missing file means no ceiling. A zero ceiling is rejected.
///
/// # Errors
///
/// Invalid JSON or a ceiling of zero or less.
pub fn load_baseline(path: &Path) -> Result<Option<Baseline>, MdError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(MdError::Io {
                context: format!("reading {}", path.display()),
                source,
            })
        }
    };
    let baseline: Baseline = serde_json::from_str(&text)
        .map_err(|e| MdError::Config(format!("parse {}: {e}", path.display())))?;
    if baseline.max_violations == 0 {
        return Err(MdError::Config(format!(
            "{} has max_violations = 0: a zero ceiling cannot express a ratchet — omit the baseline file to require zero violations",
            path.display()
        )));
    }
    Ok(Some(baseline))
}

/// Records the current total as the new ceiling.
///
/// # Errors
///
/// Write failures.
pub fn write_baseline(path: &Path, result: &LintResult, measured_at: &str) -> Result<(), MdError> {
    let baseline = Baseline {
        max_violations: result.total,
        by_rule: result.by_rule.clone(),
        measured_at: measured_at.to_owned(),
        note: "Ratchet ceiling re-measured with 'plaesy markdown validate --write-baseline'. This is the debt already in the tree, not a target: it may only go down. Fix violations (--fix, then by hand) and re-measure to lower it. Never raise it to make a red build green.".into(),
    };
    let mut text =
        serde_json::to_string_pretty(&baseline).map_err(|e| MdError::Config(e.to_string()))?;
    text.push('\n');
    fs::write(path, text).map_err(|source| MdError::Io {
        context: format!("writing {}", path.display()),
        source,
    })
}

/// Lists the Markdown files to lint, sorted and unique. With no paths, the whole
/// root is walked. Ignored directories are pruned, except the root itself.
///
/// # Errors
///
/// I/O failures walking the tree.
pub fn collect_files(
    root: &Path,
    paths: &[PathBuf],
    ignore: &[String],
) -> io::Result<Vec<PathBuf>> {
    let skip: BTreeSet<&str> = if ignore.is_empty() {
        DEFAULT_IGNORE_DIRS.iter().copied().collect()
    } else {
        ignore.iter().map(String::as_str).collect()
    };
    let mut found: BTreeSet<PathBuf> = BTreeSet::new();
    if paths.is_empty() {
        walk(root, &skip, true, &mut found)?;
    } else {
        for path in paths {
            if path.is_dir() {
                walk(path, &skip, false, &mut found)?;
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            {
                found.insert(std::path::absolute(path)?);
            } else if !path.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}", path.display()),
                ));
            }
        }
    }
    Ok(found.into_iter().collect())
}

fn walk(
    dir: &Path,
    skip: &BTreeSet<&str>,
    is_root: bool,
    found: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if skip.contains(name.as_str()) && !is_root {
                continue;
            }
            walk(&path, skip, false, found)?;
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        {
            found.insert(std::path::absolute(&path)?);
        }
    }
    Ok(())
}

/// Lints the paths (or the whole root) and reports every violation.
///
/// # Errors
///
/// Config errors, or I/O failures.
pub fn lint(
    root: &Path,
    paths: &[PathBuf],
    cfg: Option<&Config>,
    ignore: &[String],
) -> Result<LintResult, MdError> {
    let loaded;
    let cfg = match cfg {
        Some(cfg) => cfg,
        None => {
            loaded = load_config(&root.join(DEFAULT_CONFIG_NAME))?;
            &loaded
        }
    };
    let set = compile(cfg)?;
    let files = collect_files(root, paths, ignore).map_err(|source| MdError::Io {
        context: format!("walking {}", root.display()),
        source,
    })?;

    let mut result = LintResult::default();
    for path in files {
        let text = fs::read_to_string(&path).map_err(|source| MdError::Io {
            context: format!("reading {}", path.display()),
            source,
        })?;
        let rel = relative(root, &path);
        let found = check_file(&set, &text);
        result.scanned += 1;
        if found.is_empty() {
            continue;
        }
        result.files += 1;
        result.total += found.len();
        result.by_file.insert(rel.clone(), found.len());
        for v in &found {
            *result.by_rule.entry(v.rule.to_owned()).or_insert(0) += 1;
        }
        result.violations.push(FileResult {
            path: rel,
            total: found.len(),
            detail: found,
        });
    }
    result
        .violations
        .sort_by(|a, b| b.total.cmp(&a.total).then(a.path.cmp(&b.path)));
    Ok(result)
}

/// Runs every enabled rule over one document, sorted by line then rule.
#[must_use]
pub fn check_file(set: &RuleSet, text: &str) -> Vec<Violation> {
    let doc = Doc::parse(text);
    let mut out: Vec<Violation> = set
        .rules
        .iter()
        .flat_map(|rule| (rule.check)(&doc, set))
        .collect();
    out.sort_by(|a, b| a.line.cmp(&b.line).then(a.rule.cmp(b.rule)));
    out
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Applies the machine-fixable rules to every file under `paths`. Returns whether
/// any file changed.
///
/// # Errors
///
/// Config errors, or I/O failures.
pub fn fix(
    root: &Path,
    paths: &[PathBuf],
    cfg: Option<&Config>,
    ignore: &[String],
) -> Result<bool, MdError> {
    let loaded;
    let cfg = match cfg {
        Some(cfg) => cfg,
        None => {
            loaded = load_config(&root.join(DEFAULT_CONFIG_NAME))?;
            &loaded
        }
    };
    let set = compile(cfg)?;
    let files = collect_files(root, paths, ignore).map_err(|source| MdError::Io {
        context: format!("walking {}", root.display()),
        source,
    })?;
    let mut changed_any = false;
    for path in files {
        let text = fs::read_to_string(&path).map_err(|source| MdError::Io {
            context: format!("reading {}", path.display()),
            source,
        })?;
        let fixed = fix::fix_source(&set, &text);
        if fixed == text {
            continue;
        }
        fs::write(&path, fixed).map_err(|source| MdError::Io {
            context: format!("writing {}", path.display()),
            source,
        })?;
        changed_any = true;
    }
    Ok(changed_any)
}

/// Description of each implemented rule, for `--list-rules`.
#[must_use]
pub fn rule_description(id: &str) -> &'static str {
    match id {
        "MD001" => "Heading levels increment by one at a time",
        "MD003" => "Heading style is consistent (atx or setext)",
        "MD004" => "Unordered list marker style is consistent",
        "MD007" => "Unordered list indentation is a multiple of the configured width",
        "MD009" => "No trailing spaces (br_spaces tolerated)",
        "MD010" => "No hard tabs",
        "MD012" => "No more than one consecutive blank line",
        "MD013" => "Line length limit",
        "MD022" => "Headings are surrounded by blank lines",
        "MD024" => "No duplicate heading at the same level (siblings_only)",
        "MD025" => "Exactly one top-level heading",
        "MD026" => "No trailing punctuation in a heading",
        "MD029" => "Ordered list prefix style",
        "MD031" => "Fenced code blocks are surrounded by blank lines",
        "MD032" => "Lists are surrounded by blank lines",
        "MD036" => "Emphasis is not used instead of a heading",
        "MD040" => "Fenced code blocks declare a language",
        "MD041" => "First content line is a top-level heading",
        "MD046" => "Code block style is fenced",
        "MD047" => "File ends with exactly one newline",
        "MD048" => "Code fence style (backtick or tilde)",
        "MD058" => "Tables are surrounded by blank lines",
        "GHA001" => "Images have alt text",
        "GHA002" => "Alt text is not the file name",
        "GHA003" => "Link text describes the destination",
        _ => "",
    }
}
