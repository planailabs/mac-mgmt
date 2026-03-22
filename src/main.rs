use anyhow::Result;
use clap::{Parser, Subcommand};

mod crash;
mod daemon;
mod scripts;
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
    /// Run an embedded script by name
    Run {
        /// Script name (e.g. "setup.sh")
        name: String,
    },
    /// List all embedded scripts
    Scripts,
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
    let _sentry = crash::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => {
            scripts::run("setup.sh")?;
            service::install()?;
        },
        Commands::Run { name } => scripts::run(&name)?,
        Commands::Scripts => {
            for name in scripts::list() {
                println!("{name}");
            }
        }
        Commands::Install => service::install()?,
        Commands::Uninstall => service::uninstall()?,
        Commands::Daemon => daemon::run().await?,
    }

    Ok(())
}
