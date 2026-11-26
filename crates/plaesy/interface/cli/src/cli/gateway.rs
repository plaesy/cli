//! `plaesy gateway serve`: a long-running local listener for one agent name.
//!
//! It polls the local relay (`plaesy relay`) and prints each new message as it arrives, without
//! opening a chat session. Messages are shown, never acted on, and nothing is acknowledged, so the
//! person still decides what to do. The gateway opens no network port; it only reads the local
//! relay database of the repository.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use super::chat_agents;
use super::CliResult;

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Action,
}

#[derive(Debug, clap::Subcommand)]
pub enum Action {
    /// Keep polling the relay for messages to one agent name and print each new one.
    Serve(ServeArgs),
}

#[derive(Debug, clap::Args)]
pub struct ServeArgs {
    /// Agent name whose inbox is polled.
    #[arg(long)]
    pub agent: String,
    /// Repository whose relay database is read.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Seconds between polls.
    #[arg(long, default_value_t = 5)]
    pub interval: u64,
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Action::Serve(serve) => serve_forever(serve),
    }
}

fn serve_forever(serve: &ServeArgs) -> CliResult {
    let interval = Duration::from_secs(serve.interval.max(1));
    eprintln!(
        "gateway: polling relay for {} every {}s (Ctrl+C to stop)",
        serve.agent,
        interval.as_secs()
    );
    chat_agents::spawn_listener(serve.path.clone(), serve.agent.clone(), interval, |text| {
        println!("{text}");
    });
    loop {
        thread::park();
    }
}
