//! `plaesy validate`: checks a constitution, memory, Markdown, documents, design output, and the
//! instruction eval cases. With no arguments it runs the project-wide checks; with a file, the
//! checker its extension names; with a target name, that target alone.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::common::find_root;
use super::CliResult;
use crate::checklist;
use crate::designlint;
use crate::evals;
use crate::mdlint;
use crate::tasks;
use crate::validate::{self, ValidateError};

const NO_RENDER_NOTICE: &str =
    "[INFO] Structure only: no rendering check was run, so layout and visual output are not verified.";

/// The validation targets, in the order `--list` shows them.
const TARGETS: [(&str, &str); 10] = [
    (
        "contracts",
        "Check that every task contracts.json loads and is well formed",
    ),
    ("constitution", "Validate a generated constitution"),
    (
        "memory",
        "Scan .plaesy/memory/ for external references and validate self-containment",
    ),
    (
        "markdown",
        "Lint Markdown with the built-in linter (no Node, no npm)",
    ),
    (
        "assumptions",
        "Audit ASSUMED — tags left by autonomous runs, optionally gated by age",
    ),
    ("docx", "Validate a .docx file's OOXML structure"),
    ("pptx", "Validate a .pptx file's OOXML structure"),
    ("xlsx", "Validate a .xlsx file's OOXML structure"),
    (
        "design",
        "Lint produced frontend output against the design rules (offline, no browser)",
    ),
    (
        "evals",
        "Run the instruction case files: phrase checks, lint fixtures, and optional model briefs",
    ),
];

/// With no arguments, runs the project-wide checks. Otherwise see the subcommands.
#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<ValidateCommand>,
    /// List the validation targets and exit
    #[arg(long)]
    pub list: bool,
    /// A target name, or documents (.md, .docx, .pptx, .xlsx); the extension picks the checker
    pub targets: Vec<String>,
}

#[derive(Debug, clap::Subcommand)]
pub enum ValidateCommand {
    /// Validate a generated constitution
    Constitution { path: Option<PathBuf> },
    /// Scan .plaesy/memory/ for external references and validate self-containment
    Memory,
    /// Lint Markdown (ratchet baseline by default)
    Markdown(MarkdownArgs),
    /// Audit ASSUMED — tags, optionally gated by age
    Assumptions {
        /// Override the review window in days (0 = use .plaesy/state.json)
        #[arg(long = "review-days", default_value_t = 0)]
        review_days: i64,
    },
    /// Validate a .docx file's OOXML structure
    Docx { file: PathBuf },
    /// Validate a .pptx file's OOXML structure
    Pptx {
        file: PathBuf,
        /// Expected slide count
        expected: Option<usize>,
    },
    /// Validate a .xlsx file's OOXML structure
    Xlsx {
        file: PathBuf,
        /// Expected sheet names
        sheets: Vec<String>,
    },
    /// Lint produced frontend output against the design rules
    Design {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Directory of screenshots, one per required width
        #[arg(long, default_value = "")]
        evidence: String,
        /// Treat WARN findings as failures
        #[arg(long)]
        strict: bool,
    },
    /// Run the instruction case files
    Evals,
    /// Check that every task contracts.json loads and is well formed
    Contracts,
}

#[derive(Debug, clap::Args)]
pub struct MarkdownArgs {
    /// Paths to lint (default: the repository root)
    pub paths: Vec<PathBuf>,
    /// Config file (default: <repo>/.markdownlint.yaml)
    #[arg(long, default_value = "")]
    pub config: String,
    /// Ratchet baseline file (default: <repo>/.markdownlint-baseline.json)
    #[arg(long, default_value = "")]
    pub baseline: String,
    /// Ignore the baseline and require zero violations
    #[arg(long = "no-baseline")]
    pub no_baseline: bool,
    /// Print only the per-rule histogram
    #[arg(long)]
    pub summary: bool,
    /// List the implemented rules and exit
    #[arg(long = "list-rules")]
    pub list_rules: bool,
    /// Apply the machine-fixable rules in place
    #[arg(long)]
    pub fix: bool,
    /// Re-measure and record the current total as the new ceiling
    #[arg(long = "write-baseline")]
    pub write_baseline: bool,
    /// How many offending files to detail (0 = all)
    #[arg(long = "max-files", default_value_t = 20)]
    pub max_files: usize,
}

pub fn run(args: &Args) -> CliResult {
    if args.list {
        print_targets();
        return Ok(());
    }
    if let Some(command) = &args.command {
        return run_target(command);
    }
    if !args.targets.is_empty() {
        return run_files(&args.targets);
    }
    run_project()
}

fn print_targets() {
    println!("Validation targets:\n");
    for (name, short) in TARGETS {
        let scope = match name {
            "constitution" | "memory" | "markdown" | "assumptions" => "project-wide",
            "evals" => "repository",
            "contracts" => "repository",
            _ => "needs a file",
        };
        println!("  {name:<14} {scope:<14} {short}");
    }
    println!("\nBare 'plaesy validate' runs the project-wide ones. A .md file is");
    println!("routed to the markdown target; a .docx/.pptx/.xlsx to its own.");
}

fn repo_root() -> PathBuf {
    find_root().unwrap_or_else(|_| PathBuf::from("."))
}

fn skip_if_missing(path: &Path, reason: &str) -> Option<String> {
    if path.exists() {
        None
    } else {
        Some(reason.to_owned())
    }
}

/// A project-wide check: its name, why it would be skipped, and what it runs.
struct ProjectCheck {
    name: &'static str,
    skip: Option<String>,
    run: Box<dyn Fn() -> CliResult>,
}

fn run_project() -> CliResult {
    let root = repo_root();
    let constitution = root.join(".plaesy").join("memory").join("constitution.md");
    let memory_dir = root.join(".plaesy").join("memory");
    let markdown_skip = {
        let has_config = root.join(mdlint::DEFAULT_CONFIG_NAME).exists();
        let has_baseline = root.join(mdlint::DEFAULT_BASELINE_NAME).exists();
        if !has_config && !has_baseline {
            Some(format!(
                "no {} and no {} — run 'plaesy markdown validate' to opt in",
                mdlint::DEFAULT_CONFIG_NAME,
                mdlint::DEFAULT_BASELINE_NAME
            ))
        } else {
            None
        }
    };
    let tasks_dir = tasks::tasks_dir(&root);
    let project_root = root.clone();
    let checks = vec![
        ProjectCheck {
            name: "constitution",
            skip: skip_if_missing(
                &constitution,
                "no constitution yet — generate one with /start",
            ),
            run: Box::new({
                let path = constitution.clone();
                move || constitution_check(&path)
            }),
        },
        ProjectCheck {
            name: "memory",
            skip: skip_if_missing(
                &memory_dir,
                "no .plaesy/memory yet — generate one with /start",
            ),
            run: Box::new({
                let root = project_root.clone();
                move || memory_check(&root)
            }),
        },
        ProjectCheck {
            name: "markdown",
            skip: markdown_skip,
            run: Box::new({
                let root = project_root.clone();
                move || markdown_check(&root, &MarkdownArgs::defaults())
            }),
        },
        ProjectCheck {
            name: "tasks",
            skip: skip_if_missing(
                &tasks_dir,
                "no .plaesy/tasks yet — tasks are created with /implement",
            ),
            run: Box::new({
                let dir = tasks_dir.clone();
                move || {
                    let problems = tasks::check_integrity(&dir)?;
                    for p in &problems {
                        println!("[ERROR] {}: {}", p.task, p.message);
                    }
                    if problems.is_empty() {
                        println!("[OK] tasks: status matches folder, no duplicates");
                        Ok(())
                    } else {
                        Err(format!("{} task integrity problem(s)", problems.len()).into())
                    }
                }
            }),
        },
    ];

    println!("[INFO] Validating project at {}", root.display());
    let (mut passed, mut failed, mut skipped) = (0usize, 0usize, 0usize);
    for check in &checks {
        println!("\n--- {} ---", check.name);
        if let Some(reason) = &check.skip {
            skipped += 1;
            println!("[SKIP] {}: {reason}", check.name);
            continue;
        }
        if (check.run)().is_err() {
            failed += 1;
        } else {
            passed += 1;
        }
    }
    println!();
    print!("[INFO] {passed} passed, {failed} failed");
    if skipped > 0 {
        print!(", {skipped} skipped");
    }
    println!();
    if failed > 0 {
        return Err(format!("{failed} of {} project checks failed", passed + failed).into());
    }
    Ok(())
}

impl MarkdownArgs {
    fn defaults() -> Self {
        Self {
            paths: Vec::new(),
            config: String::new(),
            baseline: String::new(),
            no_baseline: false,
            summary: false,
            list_rules: false,
            fix: false,
            write_baseline: false,
            max_files: 20,
        }
    }
}

fn run_files(args: &[String]) -> CliResult {
    let mut failed = 0usize;
    for arg in args {
        let ext = Path::new(arg)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let result: CliResult = match ext.as_str() {
            "md" | "markdown" => markdown_check(&repo_root(), &MarkdownArgs {
                paths: vec![PathBuf::from(arg)],
                ..MarkdownArgs::defaults()
            }),
            "docx" => document_docx(Path::new(arg)),
            "pptx" => document_pptx(Path::new(arg), 0),
            "xlsx" => document_xlsx(Path::new(arg), &[]),
            _ => {
                return Err(format!(
                    "cannot tell what to validate from {arg:?}: pass a target ({}) or a .md/.docx/.pptx/.xlsx file",
                    TARGETS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                )
                .into())
            }
        };
        if result.is_err() {
            failed += 1;
        }
    }
    if failed > 0 {
        return Err(format!("{failed} of {} document(s) failed validation", args.len()).into());
    }
    Ok(())
}

fn run_target(command: &ValidateCommand) -> CliResult {
    let root = repo_root();
    match command {
        ValidateCommand::Constitution { path } => {
            let path = path
                .clone()
                .unwrap_or_else(|| root.join(".plaesy").join("memory").join("constitution.md"));
            constitution_check(&path)
        }
        ValidateCommand::Memory => memory_check(&root),
        ValidateCommand::Markdown(args) => markdown_check(&root, args),
        ValidateCommand::Assumptions { review_days } => assumptions_check(&root, *review_days),
        ValidateCommand::Docx { file } => document_docx(file),
        ValidateCommand::Pptx { file, expected } => document_pptx(file, expected.unwrap_or(0)),
        ValidateCommand::Xlsx { file, sheets } => document_xlsx(file, sheets),
        ValidateCommand::Design {
            paths,
            evidence,
            strict,
        } => design_check(paths, evidence, *strict),
        ValidateCommand::Evals => evals_check(&root),
        ValidateCommand::Contracts => contracts_check(&root),
    }
}

fn constitution_check(path: &Path) -> CliResult {
    println!("[INFO] Validating constitution: {}\n", path.display());
    match validate::constitution::validate(path) {
        Ok(res) => {
            println!(
                "[INFO] version: {} | active dimensions: {:?} | rules: {} | non-negotiables: {}\n",
                res.version, res.active_dimensions, res.rules, res.non_negotiables
            );
            println!("[OK] constitutional validation passed");
            println!(
                "[✓] {} is internally consistent and fully filled",
                path.display()
            );
            Ok(())
        }
        Err(e) => {
            println!("[✗] {e}");
            println!();
            println!("To fix:");
            println!("1. Regenerate from skills/plaesy/assets/constitution.template.md (plaesy init) and fill every {{{{PLACEHOLDER}}}}");
            println!(
                "2. Mark each §1 dimension row yes/no; the table is the source of the active scope"
            );
            println!("3. Mirror the 'yes' rows into active_dimensions in the frontmatter");
            println!(
                "4. Make the frontmatter version match the last amendment-log row, then re-run"
            );
            Err(e.into())
        }
    }
}

fn memory_check(root: &Path) -> CliResult {
    println!("[INFO] Scanning .plaesy/memory for external references...\n");
    let res = match validate::memory::validate(root) {
        Ok(res) => res,
        Err(e) => {
            println!("[✗] {e}");
            return Err(e.into());
        }
    };
    for issue in &res.issues {
        println!("[✗] External references in: {}", issue.file.display());
        for (pattern, lines) in &issue.matches {
            println!("  Lines with '{pattern}':");
            for line in lines {
                println!("    {line}");
            }
        }
        println!();
    }
    println!("\n[INFO] Scan complete:");
    println!("  Files checked: {}", res.files_checked);
    println!("  External refs found: {}", res.issues.len());
    println!();
    if res.issues.is_empty() {
        println!("[✓] Self-containment validated ✓");
        println!("[✓] Memory is project-local and git-safe");
        return Ok(());
    }
    println!("[!] Self-containment issues detected\n");
    println!("To fix:");
    println!("1. For content in ~/.claude/projects: Copy it into .plaesy/memory/");
    println!("2. For external links: Replace with internal .plaesy/memory/ references (flat structure, no subfolders)");
    println!("3. Re-run this command after fixes");
    Err(format!(
        "self-containment issues detected in {} file(s)",
        res.issues.len()
    )
    .into())
}

#[derive(Deserialize, Default)]
struct StateFile {
    #[serde(default)]
    assumptions_review_days: i64,
}

fn assumptions_check(root: &Path, review_days_flag: i64) -> CliResult {
    let review_days = if review_days_flag != 0 {
        review_days_flag
    } else {
        fs::read_to_string(root.join(".plaesy").join("state.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<StateFile>(&text).ok())
            .map_or(0, |s| s.assumptions_review_days)
    };
    println!("[INFO] Scanning {} for ASSUMED — tags...\n", root.display());
    let days = u32::try_from(review_days.max(0)).unwrap_or(0);
    let res = match validate::assumptions::scan(root, days) {
        Ok(res) => res,
        Err(e) => {
            println!("[✗] {e}");
            return Err(e.into());
        }
    };
    for t in &res.tags {
        println!("  {}:{} — {}", t.file.display(), t.line, t.text);
    }
    println!("\n[INFO] Scan complete:");
    println!("  Files scanned: {}", res.files_scanned);
    println!("  ASSUMED tags found: {}", res.tags.len());
    if review_days <= 0 {
        println!(
            "  Review window: off (informational only — set assumptions_review_days in .plaesy/state.json, or pass --review-days, to gate on age)\n"
        );
        println!("[✓] Audit complete (no gate configured)");
        return Ok(());
    }
    println!("  Review window: {review_days} day(s)");
    println!(
        "  Stale (file uncommitted past window): {}\n",
        res.stale.len()
    );
    if res.stale.is_empty() {
        println!("[✓] No stale assumptions");
        return Ok(());
    }
    println!("[!] Stale assumptions found:");
    for t in &res.stale {
        println!("  {}:{} — {}", t.file.display(), t.line, t.text);
    }
    Err(format!(
        "{} assumption(s) exceeded the {review_days}-day review window",
        res.stale.len()
    )
    .into())
}

fn document_error(e: ValidateError) -> Box<dyn std::error::Error> {
    println!("[✗] {e}");
    e.into()
}

fn document_docx(path: &Path) -> CliResult {
    println!("[INFO] Validating structure of {}", path.display());
    match validate::docx::validate(path) {
        Ok(res) => {
            println!(
                "[INFO] paragraphs: {}, tables: {}",
                res.paragraphs, res.tables
            );
            println!("[OK] structural validation passed");
            println!("{NO_RENDER_NOTICE}");
            println!("[✓] {} passed validation", path.display());
            Ok(())
        }
        Err(e) => Err(document_error(e)),
    }
}

fn document_pptx(path: &Path, expected: usize) -> CliResult {
    println!("[INFO] Validating structure of {}", path.display());
    match validate::pptx::validate(path, expected) {
        Ok(res) => {
            println!("[INFO] slide count: {}", res.slide_count);
            for n in &res.slides_without_title {
                println!("[WARN] slide {n} has no title placeholder");
            }
            println!("[OK] structural validation passed");
            println!("{NO_RENDER_NOTICE}");
            println!("[✓] {} passed validation", path.display());
            Ok(())
        }
        Err(e) => Err(document_error(e)),
    }
}

fn document_xlsx(path: &Path, sheets: &[String]) -> CliResult {
    println!("[INFO] Validating structure of {}", path.display());
    match validate::xlsx::validate(path, sheets) {
        Ok(res) => {
            println!("[INFO] sheets: {:?}", res.sheets);
            println!("[OK] structural validation passed");
            println!("{NO_RENDER_NOTICE}");
            println!("[✓] {} passed validation", path.display());
            Ok(())
        }
        Err(e) => Err(document_error(e)),
    }
}

fn markdown_check(root: &Path, args: &MarkdownArgs) -> CliResult {
    if args.list_rules {
        print_rule_list();
        return Ok(());
    }
    let config_path = if args.config.is_empty() {
        root.join(mdlint::DEFAULT_CONFIG_NAME)
    } else {
        PathBuf::from(&args.config)
    };
    let cfg = mdlint::load_config(&config_path).inspect_err(|e| println!("[✗] {e}"))?;
    let paths: Vec<PathBuf> = if args.paths.is_empty() {
        vec![root.to_path_buf()]
    } else {
        args.paths.clone()
    };
    if args.fix {
        let changed =
            mdlint::fix(root, &paths, Some(&cfg), &[]).inspect_err(|e| println!("[✗] {e}"))?;
        if !changed {
            println!("[OK] nothing to fix");
            return Ok(());
        }
        println!("[✓] applied machine-fixable rules (MD009, MD010, MD012, MD022, MD031, MD032, MD047, MD058)");
    }
    let res = mdlint::lint(root, &paths, Some(&cfg), &[]).inspect_err(|e| println!("[✗] {e}"))?;
    println!(
        "[INFO] scanned {} markdown file(s), {} with violations\n",
        res.scanned, res.files
    );
    let baseline_path = if args.baseline.is_empty() {
        root.join(mdlint::DEFAULT_BASELINE_NAME)
    } else {
        PathBuf::from(&args.baseline)
    };
    if args.write_baseline {
        let before = res.total;
        mdlint::write_baseline(&baseline_path, &res, &chrono::Utc::now().to_rfc3339())
            .inspect_err(|e| println!("[✗] {e}"))?;
        println!(
            "[✓] recorded a new ceiling of {before} violation(s) in {}",
            baseline_path.display()
        );
        println!("Only re-measure when the rise is understood: a ratchet that is re-set on every");
        println!("red build is not a ratchet.");
        return Ok(());
    }
    let baseline = if args.no_baseline {
        None
    } else {
        mdlint::load_baseline(&baseline_path).inspect_err(|e| println!("[✗] {e}"))?
    };
    if res.total == 0 {
        println!("[OK] no markdown violations");
        return Ok(());
    }
    if !args.summary {
        let limit = args.max_files;
        let mut shown = 0usize;
        for f in &res.violations {
            if limit > 0 && shown >= limit {
                break;
            }
            println!("{} ({})", f.path, f.total);
            for v in &f.detail {
                println!(
                    "  {}:{}:{} {} — {}",
                    f.path, v.line, v.column, v.rule, v.message
                );
            }
            shown += 1;
        }
        let remaining = res.violations.len().saturating_sub(shown);
        if remaining > 0 {
            println!("\n… and {remaining} more file(s) (see --summary)");
        }
        println!();
    }
    println!("Total: {} violation(s)", res.total);
    println!("By rule:{}", res.histogram());
    if res.exceeds(baseline.as_ref()) {
        println!();
        return match &baseline {
            None => {
                println!(
                    "[✗] {} violation(s) and --no-baseline requires zero",
                    res.total
                );
                Err(format!(
                    "markdown lint found {} violation(s) with --no-baseline",
                    res.total
                )
                .into())
            }
            Some(b) => {
                println!(
                    "[✗] {} violations exceeds the baseline ceiling of {} ({})",
                    res.total,
                    b.max_violations,
                    baseline_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                );
                println!("Either fix them (plaesy markdown validate --fix, then by hand), or record the new");
                println!(
                    "ceiling in the baseline file — never delete the baseline to make a run pass."
                );
                Err(format!(
                    "markdown lint ratchet broken: {} > {}",
                    res.total, b.max_violations
                )
                .into())
            }
        };
    }
    match &baseline {
        Some(b) => {
            let headroom = i64::try_from(b.max_violations).unwrap_or(0)
                - i64::try_from(res.total).unwrap_or(0);
            match headroom {
                0 => println!(
                    "\n[OK] exactly at the ratchet ceiling of {}. any new violation fails. Lower it with --write-baseline as you fix things. Measured {}",
                    b.max_violations, b.measured_at
                ),
                h if h < 10 => println!(
                    "\n[OK] within the ratchet ceiling of {} ({h} to spare) — measured {}",
                    b.max_violations, b.measured_at
                ),
                h => println!(
                    "\n[OK] within the ratchet ceiling of {} ({h} headroom) — measured {}",
                    b.max_violations, b.measured_at
                ),
            }
        }
        None => {
            println!();
            println!("[OK] no baseline configured, so this run required zero violations");
        }
    }
    Ok(())
}

fn print_rule_list() {
    let mut ids: Vec<&str> = mdlint::all_rules().iter().map(|r| r.id).collect();
    ids.sort();
    println!("Implemented markdownlint rules ({}):\n", ids.len());
    for id in &ids {
        println!("  {id:<8} {}", mdlint::rule_description(id));
    }
    println!();
    println!("Not implemented (markdownlint has more): MD005, MD006, MD008, MD011, MD014, MD018,");
    println!("MD019, MD020, MD021, MD023, MD027, MD028, MD030, MD033, MD034, MD035, MD037, MD038,");
    println!("MD039, MD042, MD044, MD045, MD049, MD050, MD051, MD052, MD053, MD054, MD055, MD056,");
    println!("MD059, MD060.");
    println!();
    println!(
        "A clean run means \"no implemented rule fired\", not \"markdownlint would be happy\"."
    );
    if Path::new(mdlint::DEFAULT_CONFIG_NAME).exists() {
        println!("\nConfig in use: {}", mdlint::DEFAULT_CONFIG_NAME);
    }
}

fn design_check(paths: &[PathBuf], evidence: &str, strict: bool) -> CliResult {
    let options = designlint::Options {
        evidence_dir: (!evidence.is_empty()).then(|| PathBuf::from(evidence)),
    };
    let res = designlint::lint(paths, &options).inspect_err(|e| println!("[✗] {e}"))?;
    for f in &res.findings {
        let loc = if f.line > 0 {
            format!("{}:{}", f.file, f.line)
        } else {
            f.file.clone()
        };
        println!(
            "{:<4} {loc}  {}  {}",
            f.severity.to_string(),
            f.rule,
            f.message
        );
    }
    let fails = res
        .findings
        .iter()
        .filter(|f| f.severity == designlint::Severity::Fail)
        .count();
    let warns = res
        .findings
        .iter()
        .filter(|f| f.severity == designlint::Severity::Warn)
        .count();
    println!(
        "\n[INFO] {} file(s); contrast checked {}, skipped {}",
        res.files, res.contrast_checked, res.contrast_skipped
    );
    println!("{fails} FAIL, {warns} WARN");
    if fails > 0 || (strict && warns > 0) {
        return Err(
            format!("design lint found {fails} FAIL and {warns} WARN (strict={strict})").into(),
        );
    }
    println!("[OK] design lint passed");
    Ok(())
}

fn evals_check(root: &Path) -> CliResult {
    let runner = std::env::var("PLAESY_EVAL_RUNNER").ok();
    let outcomes = evals::run(root, runner.as_deref()).inspect_err(|e| println!("[✗] {e}"))?;
    for o in &outcomes {
        let status = match o.status {
            evals::Status::Pass => "PASS",
            evals::Status::Fail => "FAIL",
            evals::Status::Skip => "SKIP",
        };
        println!(
            "{status:<4} {:<26} {:<7} {:<34} {}",
            o.file, o.kind, o.id, o.detail
        );
    }
    let (pass, fail, skip) = evals::summary(&outcomes);
    println!("\n{pass} pass, {fail} fail, {skip} skip");
    if skip > 0 {
        println!("[SKIP] {skip} brief case(s) not run: PLAESY_EVAL_RUNNER is not set. No model behaviour was checked.");
    }
    if fail > 0 {
        println!("[✗] evals failed");
        return Err(format!("{fail} eval case(s) failed").into());
    }
    println!("[OK] evals passed (static and fixture checks)");
    Ok(())
}

fn contracts_check(root: &Path) -> CliResult {
    let dir = tasks::tasks_dir(root);
    if !dir.exists() {
        println!("[SKIP] no .plaesy/tasks: nothing to check");
        return Ok(());
    }
    let refs = tasks::contracts::list_contracts(&dir, "")?;
    let mut invalid = 0usize;
    for c in &refs {
        match checklist::load(&c.path) {
            Ok(list) => println!(
                "[OK] {}/{}: {} check(s)",
                c.spec,
                c.task,
                list.checklist.len()
            ),
            Err(e) => {
                invalid += 1;
                println!("[✗] {}/{}: {e}", c.spec, c.task);
            }
        }
    }
    println!(
        "\n[INFO] {} contracts.json checked, {invalid} invalid",
        refs.len()
    );
    if invalid > 0 {
        return Err(format!("{invalid} contracts.json file(s) invalid").into());
    }
    Ok(())
}
