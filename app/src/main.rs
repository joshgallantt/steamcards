//! Entry point: parses the command line, builds the graph, runs one of the
//! two presentations.

mod composition;
mod settings;

use std::time::Duration;

use clap::Parser;

use composition::CompositionRoot;
use settings::Settings;

#[derive(Parser)]
#[command(name = "steamcards", version)]
struct Cli {
    /// Run without the TUI, printing events to stdout. Sign in with the TUI
    /// first: it shows the QR code the Steam app scans.
    #[arg(long)]
    headless: bool,

    /// Headless run duration in seconds (0 = run until interrupted).
    #[arg(long, default_value_t = 0)]
    duration: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let settings = Settings::from_environment(cli.headless)?;
    let root = CompositionRoot::new(&settings)?;

    if cli.headless {
        let duration = (cli.duration > 0).then(|| Duration::from_secs(cli.duration));
        root.presentation.run_headless(duration).await;
        return Ok(());
    }

    root.presentation.make_terminal_app().run().await
}
