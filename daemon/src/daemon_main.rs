#[cfg(feature = "self-update")]
use anyhow::Context;
use anyhow::Result;
use clap::{Parser, Subcommand};

use mac_mgmt_daemon::{
    config, crash, daemon, log_buffer, log_layer, logs, os_mgmt, scripts, service, status,
};
#[cfg(feature = "services")]
use mac_mgmt_daemon::{connectors, unmanaged};
#[cfg(feature = "self-update")]
use mac_mgmt_daemon::{nix, self_update};

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
    CheckConfig {
        /// Path to config file (defaults to the standard config path)
        #[arg(long)]
        path: Option<String>,
        /// Fetch and merge the remote server config before validating
        #[arg(long)]
        remote: bool,
    },
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
    /// Talk directly to the managed-services supervisor process
    Supervisor {
        #[command(subcommand)]
        action: SupervisorCmd,
    },
    /// Run the PII cleaner MCP server (stdio transport)
    McpCleaner(plan_ai_cleaner::Cli),
    /// Run the multi-AI cloud MCP server (stdio transport)
    McpCloud(plan_ai_cloud::Cli),
    /// Run the memvault memory MCP server (stdio transport)
    McpMemvault(plan_ai_memvault::Cli),
    /// Memvault management CLI (memctl)
    Memctl(memvault_api::memctl::Cli),
    /// systemctl compatibility shim for managed services
    Systemctl {
        /// systemctl arguments (e.g. "start ollama.service")
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum SupervisorCmd {
    /// List all registered services and their status
    List,
    /// Show detailed info for a specific service
    Info {
        /// Service name
        name: String,
    },
    /// Stop and remove a service from the supervisor
    Unregister {
        /// Service name
        name: String,
    },
    /// Request the supervisor to re-exec itself (pick up new binary)
    Reexec,
    /// Shut down the supervisor and all its children
    Shutdown,
}

fn write_ssh_fifo(command: &str) -> Result<()> {
    let path = config::config_dir().join("remote-ssh");
    std::fs::write(&path, format!("{command}\n"))
        .map_err(|e| anyhow::anyhow!("failed to write to {}: {e}", path.display()))?;
    println!("remote SSH {command}d");
    Ok(())
}

/// Lightweight entry point for argv0=systemctl mode. Skips sentry, ring,
/// and full CLI parsing for fast passthrough.
#[tokio::main]
pub async fn systemctl_main() -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match mac_mgmt_daemon::systemctl::run(args, true).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

#[tokio::main]
pub async fn main() -> ! {
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
    // Install ring as the default rustls crypto provider before any TLS
    // client is created (sentry, reqwest, etc.).
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    let _sentry = crash::init(ENVIRONMENT);
    let cli = Cli::parse();

    let result = run(cli, log_buf, filter_handle).await;
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    }
}

async fn run(
    cli: Cli,
    log_buf: log_buffer::LogBuffer,
    filter_handle: tracing_subscriber::reload::Handle<
        tracing_subscriber::EnvFilter,
        impl tracing::Subscriber,
    >,
) -> Result<()> {
    match cli.command {
        Commands::Setup => {
            scripts::run("setup.sh")?;
            service::install()?;
        }
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
        Commands::Update {
            force,
            version,
            store_path,
        } => {
            if let Some(v) = version {
                self_update::set_target(v, store_path);
            } else {
                let cfg = config::load_local()?;
                let (Some(url), Some(token)) =
                    (cfg.server.url.as_deref(), cfg.server.token.as_ref().map(|s| s.expose()))
                else {
                    anyhow::bail!("no [server] url/token configured — can't fetch update target");
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
                    anyhow::bail!("server /api/update returned {}", resp.status());
                }
                let info: mac_mgmt_common::UpdateTarget =
                    resp.json().await.context("failed to parse /api/update")?;
                let Some(ver) = info.target_version else {
                    println!("no target version configured on the server");
                    return Ok(());
                };
                self_update::set_target(ver, info.store_path);
            }
            tokio::task::spawn_blocking(move || self_update::apply(force)).await??;
        }
        #[cfg(not(feature = "self-update"))]
        Commands::Update { .. } => anyhow::bail!("self-update feature is not enabled"),
        Commands::EnableSsh => write_ssh_fifo("enable")?,
        Commands::DisableSsh => write_ssh_fifo("disable")?,
        Commands::Status { port } => status::print_status(port).await?,
        Commands::Logs {
            service,
            lines,
            follow,
        } => {
            logs::tail_logs(service.as_deref(), lines, follow, None).await?;
        }
        Commands::Sync => {
            logs::trigger_sync(None).await?;
        }
        Commands::CheckConfig { path, remote } => {
            if remote {
                let cfg = config::load().await.unwrap_or_else(|e| {
                    eprintln!("config error: {e}");
                    std::process::exit(1);
                });
                if let Err(e) = cfg.daemon.validate() {
                    eprintln!("validation error: {e}");
                    std::process::exit(1);
                }
                println!("config OK (with remote)");
            } else {
                let path = path
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(config::config_path);
                let contents = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                    eprintln!("failed to read {}: {e}", path.display());
                    std::process::exit(1);
                });
                let cfg: mac_mgmt_common::DaemonConfig =
                    toml::from_str(&contents).unwrap_or_else(|e| {
                        eprintln!("parse error: {e}");
                        std::process::exit(1);
                    });
                if let Err(e) = cfg.daemon.validate() {
                    eprintln!("validation error: {e}");
                    std::process::exit(1);
                }
                println!("config OK");
            }
        }
        Commands::Supervisor { action } => {
            let socket_path = mac_mgmt_services::default_socket_path();
            let mut client = mac_mgmt_services::Client::connect(
                &socket_path,
                std::time::Duration::from_secs(5),
            )
            .await
            .map_err(|e| anyhow::anyhow!(
                "cannot connect to supervisor at {}: {e}\n\
                 Is the daemon running?",
                socket_path.display()
            ))?;

            match action {
                SupervisorCmd::List => {
                    let services = client.list().await?;
                    if services.is_empty() {
                        println!("no services registered");
                    } else {
                        let daemon_phases: std::collections::HashMap<String, String> = {
                            let port = config::read_metrics_port();
                            let url = format!("http://[::1]:{port}/status");
                            async {
                                let client = mac_mgmt_daemon::local_client::build().ok()?;
                                let resp = client.get(&url).send().await.ok()?;
                                let s: mac_mgmt_common::StatusResponse = resp.json().await.ok()?;
                                Some(s.services.into_iter().map(|svc| (svc.name, svc.phase)).collect::<std::collections::HashMap<_, _>>())
                            }
                            .await
                            .unwrap_or_default()
                        };

                        println!("{:<20} {:<10} {:<8} {}", "NAME", "PHASE", "PID", "PROGRAM");
                        for s in &services {
                            let pid = s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
                            let program = s.spec.as_ref()
                                .map(|sp| sp.program.as_str())
                                .unwrap_or("-");
                            let phase = daemon_phases
                                .get(&s.name)
                                .map(|p| p.as_str())
                                .unwrap_or(if s.pid.is_some() { "running" } else { "stopped" });
                            println!("{:<20} {:<10} {:<8} {}", s.name, phase, pid, program);
                        }
                        println!("\n{} service(s)", services.len());
                    }
                }
                SupervisorCmd::Info { name } => {
                    let services = client.list().await?;
                    let svc = services.iter().find(|s| s.name == name);
                    match svc {
                        Some(s) => {
                            println!("Name:    {}", s.name);
                            println!("PID:     {}", s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()));
                            println!("Exe:     {}", s.exe.as_deref().unwrap_or("-"));
                            println!("Program: {}", s.resolved_program.as_deref().unwrap_or("-"));
                            if let Some(spec) = &s.spec {
                                println!("Command: {} {}", spec.program, spec.args.join(" "));
                                if !spec.env.is_empty() {
                                    println!("Env:");
                                    let mut keys: Vec<_> = spec.env.keys().collect();
                                    keys.sort();
                                    for k in keys {
                                        println!("  {}={}", k, spec.env[k]);
                                    }
                                }
                            }
                        }
                        None => {
                            eprintln!("service '{name}' not found");
                            std::process::exit(1);
                        }
                    }
                }
                SupervisorCmd::Unregister { name } => {
                    client.unregister(&name).await?;
                    println!("unregistered '{name}'");
                }
                SupervisorCmd::Reexec => {
                    client.update_self().await?;
                    println!("supervisor re-exec requested");
                }
                SupervisorCmd::Shutdown => {
                    client.shutdown().await?;
                    println!("supervisor shutdown requested");
                }
            }
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
                if let Err(e) =
                    unmanaged::installer::ensure_service(svc, &mut manifest, &manifest_path)
                {
                    tracing::error!("{}: {e:#}", svc.name());
                    failed = true;
                }
            }

            if !failed {
                let cfg = config::load().await?;
                let connectors = connectors::build_connectors(&cfg);
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
                if let Err(e) =
                    unmanaged::installer::remove_service(svc, &mut manifest, &manifest_path)
                {
                    tracing::warn!("{}: {e:#}", svc.name());
                }
            }

            let marker = config::config_dir().join(".unmanaged");
            let _ = std::fs::remove_file(marker);
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
        Commands::McpCleaner(cli) => {
            plan_ai_cleaner::run(cli).await?;
        }
        Commands::McpCloud(cli) => {
            plan_ai_cloud::run(cli).await?;
        }
        Commands::McpMemvault(cli) => {
            plan_ai_memvault::run(cli).await?;
        }
        Commands::Memctl(cli) => {
            memvault_api::memctl::run(cli).await?;
        }
        Commands::Systemctl { args } => {
            let code = mac_mgmt_daemon::systemctl::run(args, false).await?;
            std::process::exit(code);
        }
    }

    Ok(())
}
