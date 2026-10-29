//! `plaesy config`: reads and validates `configs/platform.json`.

use std::path::PathBuf;

use super::common::{config_path, load_platform_config};
use super::CliResult;
use crate::config::platform::{clean_files, normalize_platform, PlatformConfig};

/// Reads and validates `configs/platform.json`. With no subcommand, prints the subcommands.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Path to platform.json (default: configs/platform.json in the Plaesy home)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum ConfigCommand {
    /// Get a mapping value (the value/excludes structure)
    GetMapping {
        section: String,
        mapping_type: String,
    },
    /// Get the exclude patterns for a mapping
    GetExcludes {
        section: String,
        mapping_type: String,
    },
    /// Get the vendor-style files to ignore
    GetCleanFiles,
    /// Get the build directories to ignore for a platform
    GetCleanDirs { platform: Option<String> },
    /// Get one component of the .plaesy/ layout
    GetStructure { component: String },
    /// Check that platform.json is well-formed
    Validate,
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("config");
    };
    let explicit = args.config.as_deref();
    match command {
        ConfigCommand::GetCleanFiles => {
            println!("{}", clean_files().join(" "));
        }
        ConfigCommand::Validate => {
            let path = config_path(explicit)?;
            PlatformConfig::load(&path)?;
            println!("Platform configuration is valid");
        }
        ConfigCommand::GetMapping {
            section,
            mapping_type,
        } => {
            let (cfg, _) = load_platform_config(explicit)?;
            let mut value = cfg.mapping_value(section).to_owned();
            if value.is_empty() {
                value = cfg.platform_field(section, &format!("mapping.{mapping_type}"))?;
            }
            println!("{value}");
        }
        ConfigCommand::GetExcludes { section, .. } => {
            let (cfg, _) = load_platform_config(explicit)?;
            for e in cfg.mapping_excludes(section) {
                println!("{e}");
            }
        }
        ConfigCommand::GetCleanDirs { platform } => {
            let (cfg, _) = load_platform_config(explicit)?;
            let id = match platform {
                Some(p) => Some(normalize_platform(p, Some(&cfg))),
                None => Some(
                    cfg.detect_platform()
                        .map(str::to_owned)
                        .ok_or("no platform detected")?,
                ),
            };
            let dirs = cfg.clean_dirs(id.as_deref())?;
            println!("{}", dirs.join(" "));
        }
        ConfigCommand::GetStructure { component } => {
            let (cfg, _) = load_platform_config(explicit)?;
            println!("{}", cfg.structure_component(component)?);
        }
    }
    Ok(())
}
