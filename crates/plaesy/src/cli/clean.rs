//! `plaesy clean`: removes Plaesy framework files and directories, with a plan and confirmation.

use std::io::{self, BufReader, Write};
use std::path::PathBuf;
use std::str::FromStr;

use super::common::{banner, logger};
use super::CliResult;
use crate::cleaner::{Cleaner, Level, Options};
use crate::config::platform::{normalize_platform, PlatformConfig};
use crate::installer;
use crate::scaffold::config::platform_config_path;

/// Cleanup levels: `safe` (framework files, default), `thorough` (also the platform mappings),
/// `complete` (also the generic AI files). Use `--dry-run` first to preview.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Project to clean (default: current directory)
    pub target_dir: Option<PathBuf>,
    /// Skip the confirmation prompt
    #[arg(short = 'y', long = "yes")]
    pub auto_confirm: bool,
    /// Show what would be removed without removing it
    #[arg(long)]
    pub dry_run: bool,
    /// Create a backup before removal (default: on)
    #[arg(long, value_name = "BOOL")]
    pub backup: Option<bool>,
    /// Skip backup creation
    #[arg(long = "no-backup")]
    pub no_backup: bool,
    /// Cleanup level: safe, thorough, or complete
    #[arg(long, default_value = "safe")]
    pub level: String,
    /// AI platform to clean (auto-detected when not given)
    #[arg(long, default_value = "")]
    pub ai: String,
    /// Show detailed progress
    #[arg(long)]
    pub verbose: bool,
    /// Path to platform.json (default: configs/platform.json in the Plaesy home)
    #[arg(long)]
    pub config: Option<PathBuf>,
}

fn guidance(ai: &str) -> &'static str {
    match ai.split_whitespace().next().unwrap_or("") {
        "claude" | "claude_code" => "For Claude Code: Use /clean command in future",
        "cursor" | "cursor_ai" => "For Cursor AI: Use --clean command in future",
        "windsurf" | "windsurf_ai" => "For Windsurf AI: Use clean command in future",
        "copilot" | "github_copilot" => "For GitHub Copilot: Continue using plaesy clean command",
        _ => "For other platforms: Use plaesy clean command",
    }
}

pub fn run(args: &Args) -> CliResult {
    let log = logger();
    let level = Level::from_str(&args.level).map_err(|_| {
        format!(
            "invalid cleanup level: {} (available levels: safe, thorough, complete)",
            args.level
        )
    })?;

    // `--backup` defaults to on, so only an explicit `--no-backup` or `--backup=false` turns it off.
    let backup = if args.no_backup {
        false
    } else {
        args.backup.unwrap_or(true)
    };

    let target_dir = args
        .target_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = match &args.config {
        Some(p) => p.clone(),
        None => platform_config_path(&installer::plaesy_home_dir()?),
    };
    let config = PlatformConfig::load(&config_path)
        .map_err(|e| format!("configuration validation failed: {e}"))?;

    let mut ai_choice = String::new();
    if !args.ai.is_empty() {
        // Resolve the shorthand to the id platform.json declares before any lookup by key.
        ai_choice = normalize_platform(&args.ai, Some(&config));
        if !config.has_platform(&ai_choice) {
            return Err(format!(
                "invalid AI assistant: {} (available assistants: {:?})",
                args.ai,
                config.platform_ids()
            )
            .into());
        }
    }

    let options_for = |ai: String| Options {
        target_dir: target_dir.clone(),
        auto_confirm: args.auto_confirm,
        dry_run: args.dry_run,
        backup,
        level,
        ai_choice: ai,
    };
    let mut cleaner = Cleaner::new(options_for(ai_choice.clone()), config, config_path.clone());

    if args.verbose {
        println!("Configuration:");
        println!("  Cleanup Level: {}", args.level);
        println!("  Dry Run: {}", args.dry_run);
        println!("  Backup: {backup}");
        println!(
            "  Platform: {}",
            if ai_choice.is_empty() {
                "auto-detect"
            } else {
                ai_choice.as_str()
            }
        );
        println!("  Target Directory: {}", target_dir.display());
        println!();
    }

    if ai_choice.is_empty() {
        println!("Detecting AI platforms for cleanup...");
        let platforms = cleaner.detect_all_platforms()?;
        let resolved = match platforms.as_slice() {
            [only] if only != "generic_ai" => {
                println!("Detected AI platform: {only}");
                println!("Using platform-adapted cleanup...");
                only.clone()
            }
            [_, _, ..] => {
                println!("Detected multiple AI platforms: {}", platforms.join(", "));
                println!("Using multi-platform cleanup...");
                platforms.join(" ")
            }
            _ => {
                println!("No specific AI platform detected. Using generic cleanup...");
                // Keep the config's own fallback id. The literal "generic" is the sentinel
                // that the plan skips, which would spare generic_ai files at every level.
                platforms.first().cloned().unwrap_or_default()
            }
        };
        cleaner = Cleaner::new(
            options_for(resolved.clone()),
            PlatformConfig::load(&config_path)?,
            config_path.clone(),
        );
        ai_choice = resolved;
    } else {
        println!("Using specified AI: {ai_choice}");
    }

    banner("Plaesy Clean", "Remove plaesy framework directories")?;
    cleaner.validate_environment()?;

    let plan = cleaner.plan()?;
    let stdout = io::stdout();
    cleaner.print_plan(&plan, &mut stdout.lock())?;
    if plan.is_empty() {
        log.success("No Plaesy directories or files found to remove.");
        return Ok(());
    }

    let mut input = BufReader::new(io::stdin().lock());
    if !cleaner.confirm(&mut input, &mut stdout.lock())? {
        println!("Clean operation cancelled.");
        return Ok(());
    }

    if backup && !args.dry_run {
        cleaner.create_backup(&log)?;
    }
    if !args.dry_run {
        cleaner.remove(&log, &mut stdout.lock())?;
    }

    println!();
    log.success("🧹 Plaesy clean completed!");

    if !ai_choice.is_empty() {
        println!();
        println!("Platform-specific cleanup guidance:");
        println!("{}", guidance(&ai_choice));
    }
    println!();
    println!("Cleaned directory: {}", target_dir.display());
    println!();
    println!("To reinitialize the project:");
    println!("   plaesy init [--ai=your-choice]");
    println!();
    io::stdout().flush()?;
    Ok(())
}
