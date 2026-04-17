use anyhow::Result;
#[cfg(feature = "self-update")]
use anyhow::Context;
use clap::{Parser, Subcommand};

mod assessment;
mod cmd;
mod config;
mod config_watch;
#[cfg(feature = "services")]
mod connectors;
mod crash;
mod daemon;
mod host_keys;
mod log_buffer;
mod log_layer;
mod events;
mod logs;
#[cfg(feature = "services")]
mod config_providers;
#[cfg(feature = "services")]
mod managed_service;
mod mcp_servers;
mod metrics;
mod metrics_server;
mod notify;
mod sentry_ext;
mod nix;
mod os_mgmt;
#[cfg(feature = "relay")]
mod file_tunnels;
#[cfg(feature = "relay")]
mod remote_ssh;
mod scripts;
#[cfg(feature = "self-update")]
mod self_update;
#[cfg(feature = "services")]
mod service_mgmt;
mod skills;
mod service;
mod server_push;
mod status;
mod ws_reconnect;
#[cfg(feature = "services")]
mod services;
#[cfg(feature = "services")]
mod unmanaged;

/// Git commit this binary was built from. Captured at build time by
/// build.rs (GIT_SHA env or `git rev-parse HEAD`); "unknown" when
/// neither is available.
pub const GIT_SHA: &str = env!("GIT_SHA");

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
        /// Override the target version (used by integration tests).
        #[arg(long)]
        version: Option<String>,
        /// Override the nix store path containing `bin/mac-mgmt` for the
        /// override version. Used by integration tests to exercise the
        /// `nix-store --realise` + self-replace flow without a server.
        #[arg(long)]
        store_path: Option<String>,
    },
    /// Validate the config file and exit
    CheckConfig,
    /// Show daemon and service status
    Status {
        /// Metrics port (reads from config if omitted)
        #[arg(long)]
        port: Option<u16>,
    },
    /// Enable remote SSH access via the relay
    EnableSsh,
    /// Disable remote SSH access via the relay
    DisableSsh,
    /// Trigger an immediate sync of skills, MCP servers, and SSH keys
    Sync,
    /// Run the managed-services supervisor (started by the mac-mgmt-services
    /// system unit — not normally invoked by hand).
    Services,
    /// View service logs
    Logs {
        /// Service name (e.g., "ollama"). Shows all services if omitted.
        #[arg(short, long)]
        service: Option<String>,
        /// Number of lines to show (default: 50)
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
        /// Follow log output (like tail -f)
        #[arg(short, long)]
        follow: bool,
    },
    /// Install services as standalone system daemons (unmanaged mode —
    /// services are run by systemd/launchd, not the mac-mgmt supervisor).
    /// Reads config.toml to determine which services to install.
    #[cfg(feature = "services")]
    InstallServices,
    /// Uninstall services previously installed via `install-services`.
    #[cfg(feature = "services")]
    UninstallServices,
    /// Detect existing service installations and adopt them into the
    /// unmanaged manifest without modifying anything on disk.
    #[cfg(feature = "services")]
    ImportServices,
}

fn write_ssh_fifo(command: &str) -> Result<()> {
    let path = config::config_dir().join("remote-ssh");
    std::fs::write(&path, format!("{command}\n"))
        .map_err(|e| anyhow::anyhow!("failed to write to {}: {e}", path.display()))?;
    println!("remote SSH {command}d");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let log_buf = log_buffer::LogBuffer::new();

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let (filter_layer, filter_handle) = tracing_subscriber::reload::Layer::new(env_filter);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(tracing_subscriber::fmt::layer())
        .with(log_layer::BufferLayer::new(log_buf.clone()))
        .init();

    const ENVIRONMENT: &str = match option_env!("ENVIRONMENT") {
        Some(v) => v,
        None => "dev",
    };
    let _sentry = crash::init(ENVIRONMENT);
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
        Commands::Daemon => {
            let set_log_level = Box::new(move |level: &str| {
                if let Ok(filter) = level.parse::<tracing_subscriber::EnvFilter>() {
                    let _ = filter_handle.reload(filter);
                }
            });
            daemon::run(log_buf, set_log_level).await?
        }
        Commands::Services => {
            #[cfg(feature = "services")]
            {
                let socket_path = mac_mgmt_services::default_socket_path();
                let reexec = mac_mgmt_services::server::run(&socket_path).await?;
                if reexec {
                    mac_mgmt_services::server::reexec_self();
                }
            }
            #[cfg(not(feature = "services"))]
            anyhow::bail!("services feature is not enabled");
        }
        Commands::ConfigureOs { dry_run } => os_mgmt::configure_os(dry_run)?,
        #[cfg(feature = "self-update")]
        Commands::Update { force, version, store_path } => {
            if let Some(v) = version {
                // Explicit override path — mostly used by integration tests
                // that exercise the store-path self-replace flow without a
                // live server.
                self_update::set_target(v, store_path);
            } else {
                // CLI runs in a fresh process with an empty in-process
                // target. Populate it from the server the same way the
                // daemon's update tick does, otherwise apply() prints
                // "no target version known" and no-ops.
                let cfg = config::load().await?;
                let (Some(url), Some(token)) =
                    (cfg.server.url.as_deref(), cfg.server.token.as_deref())
                else {
                    anyhow::bail!(
                        "no [server] url/token configured — can't fetch update target"
                    );
                };
                let system = nix::current_system().unwrap_or("");
                let client = reqwest::Client::new();
                let resp = client
                    .get(format!("{url}/api/update?system={system}"))
                    .bearer_auth(token)
                    .send()
                    .await
                    .context("failed to fetch /api/update")?;
                if !resp.status().is_success() {
                    anyhow::bail!(
                        "server /api/update returned {}",
                        resp.status()
                    );
                }
                let info: mac_mgmt_common::UpdateTarget =
                    resp.json().await.context("failed to parse /api/update")?;
                let Some(ver) = info.target_version else {
                    println!("no target version configured on the server");
                    return Ok(());
                };
                self_update::set_target(ver, info.store_path);
            }
            tokio::task::spawn_blocking(move || self_update::apply(force))
                .await??;
        }
        #[cfg(not(feature = "self-update"))]
        Commands::Update { .. } => anyhow::bail!("self-update feature is not enabled"),
        Commands::EnableSsh => write_ssh_fifo("enable")?,
        Commands::DisableSsh => write_ssh_fifo("disable")?,
        Commands::Status { port } => status::print_status(port).await?,
        Commands::Logs { service, lines, follow } => {
            logs::tail_logs(service.as_deref(), lines, follow, None).await?;
        }
        Commands::Sync => {
            logs::trigger_sync(None).await?;
        }
        Commands::CheckConfig => {
            let cfg = config::load().await.map_err(|e| {
                eprintln!("config error: {e}");
                std::process::exit(1);
            }).unwrap();
            if let Err(e) = cfg.daemon.validate() {
                eprintln!("config error: {e}");
                std::process::exit(1);
            }
            println!("config OK");
        }
        #[cfg(feature = "services")]
        Commands::InstallServices => {
            let _lock = unmanaged::installer::acquire_lock()?;
            let mut cfg = config::load().await?;
            let manifest_path = unmanaged::manifest::InstallManifest::path();
            let mut manifest = unmanaged::manifest::InstallManifest::load(&manifest_path)?;
            let services = unmanaged::build_unmanaged(&mut cfg);

            let marker = config::config_dir().join(".unmanaged");
            std::fs::create_dir_all(config::config_dir())?;
            std::fs::write(&marker, "")?;

            let mut failed = false;
            for svc in &services {
                if let Err(e) = unmanaged::installer::ensure_service(svc, &mut manifest, &manifest_path) {
                    tracing::error!("{}: {e:#}", svc.name());
                    failed = true;
                }
            }

            // Run connectors after all services are installed.
            if !failed {
                let cfg = config::load().await?;
                let connectors = connectors::build_connectors(
                    &cfg.global,
                    &cfg.ollama,
                    &cfg.lms,
                    &cfg.cloud,
                );
                let configs = std::collections::HashMap::new();
                for c in &connectors {
                    if let Err(e) = c.connect(&configs) {
                        tracing::warn!("connector {}: {e:#}", c.name());
                    }
                }
                manifest.connectors_applied = true;
                manifest.save(&manifest_path)?;
            }

            if failed {
                anyhow::bail!("some services failed to install (re-run to retry)");
            }
            println!("all services installed");
        }
        #[cfg(feature = "services")]
        Commands::UninstallServices => {
            let _lock = unmanaged::installer::acquire_lock()?;
            let manifest_path = unmanaged::manifest::InstallManifest::path();
            let mut manifest = unmanaged::manifest::InstallManifest::load(&manifest_path)?;
            let mut cfg = config::load().await?;
            let services = unmanaged::build_unmanaged(&mut cfg);

            for svc in services.iter().rev() {
                if let Err(e) = unmanaged::installer::remove_service(svc, &mut manifest, &manifest_path) {
                    tracing::warn!("{}: {e:#}", svc.name());
                }
            }

            // Remove .unmanaged marker.
            let marker = config::config_dir().join(".unmanaged");
            let _ = std::fs::remove_file(marker);

            // Clean up manifest.
            manifest.connectors_applied = false;
            manifest.save(&manifest_path)?;

            println!("all services uninstalled");
        }
        #[cfg(feature = "services")]
        Commands::ImportServices => {
            let _lock = unmanaged::installer::acquire_lock()?;
            let manifest_path = unmanaged::manifest::InstallManifest::path();
            let mut manifest = unmanaged::manifest::InstallManifest::load(&manifest_path)?;
            let mut cfg = config::load().await?;
            let services = unmanaged::build_unmanaged(&mut cfg);

            let mut found = 0;
            for svc in &services {
                match unmanaged::installer::import_service(svc, &mut manifest, &manifest_path) {
                    Ok(true) => {
                        let s = manifest.services.get(svc.name());
                        let pkg = s.map(|s| s.package_installed).unwrap_or(false);
                        let cfg_ok = s.map(|s| s.configured).unwrap_or(false);
                        let active = s.map(|s| s.service_active).unwrap_or(false);
                        println!(
                            "  ✔ {:<15} package={} config={} service={}",
                            svc.name(),
                            if pkg { "found" } else { "missing" },
                            if cfg_ok { "found" } else { "missing" },
                            if active { "active" } else { "inactive" },
                        );
                        found += 1;
                    }
                    Ok(false) => {
                        println!("  · {:<15} not found", svc.name());
                    }
                    Err(e) => {
                        tracing::warn!("{}: import failed: {e:#}", svc.name());
                    }
                }
            }
            println!("{found} service(s) imported");
        }
    }

    Ok(())
}
