use anyhow::Result;
use clap::{Parser, Subcommand};

mod config;
mod crash;
mod daemon;
mod managed_service;
mod metrics;
mod metrics_server;
mod sentry_ext;
mod nix;
mod scripts;
mod skills;
mod service;
mod services;

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
    /// Uninstall the service
    Uninstall,
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Restart the service
    Restart,
    /// Run the daemon (called by launchd)
    Daemon,
    /// Check for updates and apply if available
    Update {
        /// Force update even if already on the latest version
        #[arg(long)]
        force: bool,
    },
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
        Commands::Start => service::start()?,
        Commands::Stop => service::stop()?,
        Commands::Restart => service::restart()?,
        Commands::Daemon => daemon::run().await?,
        Commands::Update { force } => daemon::do_update(force)?,
    }

    Ok(())
}
