//! `plaesy voice`: parses a transcribed sentence into a task command. `parse` only prints the
//! result as JSON; it never creates, completes, or lists tasks. Audio capture is not included.
use super::CliResult;
use clap::{Args as ClapArgs, Subcommand};

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse transcribed text into a task command and print it as JSON (nothing is executed)
    Parse {
        /// The transcribed sentence, for example "buat tugas beli susu"
        text: String,
    },
}

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("voice");
    };
    match command {
        Command::Parse { text } => {
            let parsed = crate::voice::parse(text)?;
            println!("{}", serde_json::to_string_pretty(&parsed)?);
        }
    }
    Ok(())
}
