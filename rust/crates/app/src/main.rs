mod cli;
// `new_sidecar` is used only by the meeting-server bin.
#[allow(dead_code)]
mod container;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    cli::Cli::parse().run().await
}
