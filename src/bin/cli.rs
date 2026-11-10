//! Thin standalone entry point for the in-process memory CLI.

#[tokio::main]
async fn main() {
    if let Err(error) = plaesy::memory::cli::run_args(std::env::args().skip(1).collect()).await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
