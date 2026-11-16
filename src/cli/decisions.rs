//! CLI for Plaesy-owned structured decision inference.

use clap::{Args as ClapArgs, Subcommand};

use super::CliResult;

#[derive(Debug, ClapArgs)]
#[command(arg_required_else_help = true)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect or update local decision-model configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Manage downloadable local model artifacts
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    /// Choose one option using the configured local decision model
    Choose {
        /// The state or case to evaluate
        #[arg(long)]
        state: String,
        /// What the decision should optimize for
        #[arg(long)]
        prompt: String,
        /// Candidate value (repeat at least twice)
        #[arg(long = "option", required = true, num_args = 1, action = clap::ArgAction::Append)]
        options: Vec<String>,
        /// Print the complete structured model response as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Show effective decision-model configuration
    Show,
    /// Select the supported model
    Set {
        /// Currently supported model: laya
        #[arg(long, default_value = "laya")]
        model: String,
    },
    /// Restore the default configuration
    Reset,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommand {
    /// List supported models and local installation state
    List,
    /// Show the pinned source and artifact metadata
    Info {
        /// Model identifier
        #[arg(default_value = "laya")]
        model: String,
    },
    /// Download and verify a pinned model revision
    Pull {
        /// Model identifier
        #[arg(default_value = "laya")]
        model: String,
    },
    /// Remove this model's files; configuration is retained
    Remove {
        /// Model identifier
        #[arg(default_value = "laya")]
        model: String,
    },
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Command::Config { command } => match command {
            ConfigCommand::Show => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&crate::decisions::load_config()?)?
                );
                Ok(())
            }
            ConfigCommand::Set { model } => {
                crate::decisions::save_config(&crate::decisions::DecisionsConfig {
                    model: model.clone(),
                })?;
                println!("Configured decision model: {model}");
                Ok(())
            }
            ConfigCommand::Reset => {
                crate::decisions::save_config(&crate::decisions::DecisionsConfig::default())?;
                println!("Decision-model configuration reset to defaults");
                Ok(())
            }
        },
        Command::Model { command } => match command {
            ModelCommand::List => {
                let installed = crate::decisions::model_status()?;
                println!(
                    "laya\t{}\t{}",
                    if installed {
                        "installed"
                    } else {
                        "not installed"
                    },
                    crate::decisions::REVISION
                );
                Ok(())
            }
            ModelCommand::Info { model } => {
                ensure_laya(model)?;
                println!(
                    "id: laya\nsource: https://huggingface.co/convaiinnovations/laya\nrevision: {}\nlicense: Apache-2.0\nruntime: laya-decision 0.2.20 (Rust/Candle)\nfiles:",
                    crate::decisions::REVISION
                );
                for (name, size, hash) in crate::decisions::FILES {
                    println!("  {name}  size={size} sha256={hash}");
                }
                Ok(())
            }
            ModelCommand::Pull { model } => {
                ensure_laya(model)?;
                crate::decisions::pull()?;
                Ok(())
            }
            ModelCommand::Remove { model } => {
                ensure_laya(model)?;
                crate::decisions::remove()?;
                Ok(())
            }
        },
        Command::Choose {
            state,
            prompt,
            options,
            json,
        } => {
            let result = crate::decisions::choose(state, prompt, options)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                let answer = result
                    .pointer("/answers/recommendation")
                    .ok_or("Laya result missing recommendation")?;
                println!("{}", serde_json::to_string_pretty(answer)?);
            }
            Ok(())
        }
    }
}

fn ensure_laya(model: &str) -> Result<(), Box<dyn std::error::Error>> {
    if model != "laya" {
        return Err(
            format!("unsupported model {model:?}; currently only laya is available").into(),
        );
    }
    Ok(())
}
