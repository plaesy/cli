//! `plaesy platforms`: the AI platforms plaesy can target.

use std::path::PathBuf;

use super::common::load_platform_config;
use super::CliResult;
use crate::config::platform::normalize_platform;

/// Work with the AI platform a project targets. With no subcommand, prints the subcommands.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Path to platform.json (default: configs/platform.json in the Plaesy home)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<PlatformsCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum PlatformsCommand {
    /// Detect the AI platform this project is on
    Detect,
    /// List every platform plaesy knows about
    List,
    /// Show one platform in detail
    Show {
        /// Platform id or alias (default: detected)
        platform: Option<String>,
    },
    /// Get a platform's configuration value
    Get { platform: String, key: String },
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("platforms");
    };
    let (cfg, _path) = load_platform_config(args.config.as_deref())?;
    match command {
        PlatformsCommand::Detect => {
            let id = cfg.detect_platform().ok_or("no platform detected")?;
            println!("{id}");
        }
        PlatformsCommand::List => {
            for id in cfg.platform_ids() {
                println!("{id}");
            }
        }
        PlatformsCommand::Get { platform, key } => {
            let id = normalize_platform(platform, Some(&cfg));
            println!("{}", cfg.platform_field(&id, key)?);
        }
        PlatformsCommand::Show { platform } => {
            let id = match platform {
                Some(p) => normalize_platform(p, Some(&cfg)),
                None => cfg
                    .detect_platform()
                    .map(str::to_owned)
                    .ok_or("no platform detected")?,
            };
            println!("Platform Information for: {id}");
            println!("================================");
            match cfg.platform(&id) {
                Err(_) => {
                    println!("Name: Unknown");
                    println!("Provider: Unknown");
                    println!("Category: Unknown");
                }
                Ok(p) => {
                    for (label, value) in [
                        ("Name", &p.name),
                        ("Provider", &p.provider),
                        ("Category", &p.category),
                    ] {
                        let shown = if value.is_empty() {
                            "Unknown"
                        } else {
                            value.as_str()
                        };
                        println!("{label}: {shown}");
                    }
                }
            }
        }
    }
    Ok(())
}
