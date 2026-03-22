use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process::Command;

mod daemon;
mod service;

#[derive(Parser)]
#[command(name = "mac-mgmt", version, about = "Mac management daemon")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the setup script
    Setup,
    /// Install the launchd service
    Install,
    /// Uninstall the launchd service
    Uninstall,
    /// Run the daemon (called by launchd)
    Daemon,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => run_setup()?,
        Commands::Install => service::install()?,
        Commands::Uninstall => service::uninstall()?,
        Commands::Daemon => daemon::run().await?,
    }

    Ok(())
}

fn run_setup() -> Result<()> {
    let script = std::env::current_exe()?
        .parent()
        .context("no parent dir")?
        .join("../share/mac-mgmt/setup.sh");

    // Fall back to setup.sh next to the binary or in cwd
    let script = if script.exists() {
        script
    } else {
        let next_to_bin = std::env::current_exe()?
            .parent()
            .context("no parent dir")?
            .join("setup.sh");
        if next_to_bin.exists() {
            next_to_bin
        } else {
            std::path::PathBuf::from("setup.sh")
        }
    };

    tracing::info!("running setup script: {}", script.display());

    let status = Command::new("sh")
        .arg(&script)
        .status()
        .context("failed to run setup.sh")?;

    if !status.success() {
        anyhow::bail!("setup.sh exited with status {}", status);
    }

    Ok(())
}
