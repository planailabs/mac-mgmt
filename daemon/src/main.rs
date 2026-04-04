use anyhow::Result;
use clap::{Parser, Subcommand};

mod config;
mod crash;
mod daemon;
mod instance_id;
mod managed_service;
mod mcp_servers;
mod metrics;
mod metrics_server;
mod sentry_ext;
mod nix;
mod os_mgmt;
mod remote_ssh;
mod scripts;
#[cfg(feature = "self-update")]
mod self_update;
#[cfg(feature = "services")]
mod service_mgmt;
mod skills;
mod ssh_keys;
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
    /// Install OS-specific configuration files
    ConfigureOs {
        /// Preview changes without writing files
        #[arg(long)]
        dry_run: bool,
    },
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
        Commands::ConfigureOs { dry_run } => os_mgmt::configure_os(dry_run)?,
        #[cfg(feature = "self-update")]
        Commands::Update { force } => self_update::apply(force)?,
        #[cfg(not(feature = "self-update"))]
        Commands::Update { .. } => anyhow::bail!("self-update feature is not enabled"),
    }

    Ok(())
}
