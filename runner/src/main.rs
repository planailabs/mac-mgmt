//! mac-mgmt-runner: orchestrator daemon + CLI client.
//!
//! In `daemon` mode it runs a reconcile loop against a configured Incus host
//! and mac-mgmt server, materialising a matrix of test daemons. It also
//! exposes a local HTTP API so the remaining CLI subcommands can drive it.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod api;
mod config;
mod host_key;
mod incus;
mod matrix;
mod mgmt;
mod orchestrator;
mod state;

use crate::config::RunnerConfig;
use crate::incus::IncusClient;
use crate::mgmt::MgmtClient;
use crate::orchestrator::{Orchestrator, deploy_watchdog_loop, reconcile_loop, reprovision_loop};

#[derive(Parser)]
#[command(name = "mac-mgmt-runner", version, about = "mac-mgmt fleet runner")]
struct Opts {
    /// Path to runner config TOML.
    #[arg(long, short = 'c', default_value = "/etc/mac-mgmt-runner/config.toml", env = "MAC_MGMT_RUNNER_CONFIG")]
    config: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the orchestrator daemon + HTTP API.
    Daemon,
    /// Show fleet status (dials the running daemon).
    Status {
        /// Print JSON instead of a human summary.
        #[arg(long)]
        json: bool,
    },
    /// Ensure every matrix cell is provisioned.
    Provision,
    /// Destroy every provisioned cell.
    Teardown {
        /// Skip the confirmation prompt.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Reprovision a specific cell or a random one.
    Reprovision {
        /// Matrix cell key (e.g. "openclaw-ollama"). If omitted, a random cell is picked.
        key: Option<String>,
    },
    /// Print the matrix of ClusterConfig values that would be generated.
    Matrix {
        /// Print JSON per cell instead of keys only.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,mac_mgmt_runner=info")),
        )
        .init();

    let opts = Opts::parse();
    let cfg = RunnerConfig::load(&opts.config)
        .with_context(|| format!("loading runner config from {}", opts.config.display()))?;

    // Sentry must be initialised before the tokio runtime so the panic hook is in place.
    let _sentry_guard = init_sentry(&cfg);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;

    rt.block_on(async move {
        match opts.cmd {
            Cmd::Daemon => run_daemon(cfg).await,
            Cmd::Status { json } => cmd_status(&cfg, json).await,
            Cmd::Provision => cmd_provision(&cfg).await,
            Cmd::Teardown { yes } => cmd_teardown(&cfg, yes).await,
            Cmd::Reprovision { key } => cmd_reprovision(&cfg, key.as_deref()).await,
            Cmd::Matrix { json } => cmd_matrix(&cfg, json),
        }
    })
}

fn init_sentry(cfg: &RunnerConfig) -> Option<sentry::ClientInitGuard> {
    let dsn = cfg.sentry.dsn.as_deref()?;
    Some(sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: cfg.sentry.environment.clone().map(Into::into),
            ..Default::default()
        },
    )))
}

async fn run_daemon(cfg: RunnerConfig) -> Result<()> {
    let orch = build_orchestrator(cfg).await?;
    orch.mgmt
        .preflight()
        .await
        .context("mgmt preflight failed — check mgmt.url and admin_token")?;
    let orch = Arc::new(orch);

    // Initial reconcile is kicked off in the background so the HTTP API comes up immediately.
    tokio::spawn({
        let orch = orch.clone();
        async move {
            if let Err(e) = orch.reconcile().await {
                tracing::error!("initial reconcile: {e:#}");
                sentry::integrations::anyhow::capture_anyhow(&e);
            }
        }
    });

    tokio::spawn(reconcile_loop(orch.clone()));
    tokio::spawn(deploy_watchdog_loop(orch.clone()));
    tokio::spawn(reprovision_loop(orch.clone()));

    api::serve(orch).await
}

async fn build_orchestrator(cfg: RunnerConfig) -> Result<Orchestrator> {
    let mgmt = MgmtClient::new(&cfg.mgmt.url, &cfg.mgmt.admin_token, cfg.mgmt.organization_id)?;

    let cert = std::fs::read(&cfg.incus.client_cert)
        .with_context(|| format!("reading {}", cfg.incus.client_cert.display()))?;
    let key = std::fs::read(&cfg.incus.client_key)
        .with_context(|| format!("reading {}", cfg.incus.client_key.display()))?;
    let mut combined = cert;
    combined.extend_from_slice(b"\n");
    combined.extend_from_slice(&key);

    let ca = match &cfg.incus.server_ca {
        Some(p) => Some(
            std::fs::read(p).with_context(|| format!("reading {}", p.display()))?,
        ),
        None => None,
    };
    let incus = IncusClient::new(&cfg.incus.url, &cfg.incus.project, &combined, ca.as_deref())?;

    Orchestrator::new(cfg, mgmt, incus)
}

async fn cmd_status(cfg: &RunnerConfig, json: bool) -> Result<()> {
    let cli = api::Cli::new(&cfg.api.bind, cfg.api.port);
    let snap = cli.status().await.context("dialing runner daemon")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
    } else {
        println!(
            "matrix: {} cells, {} provisioned",
            snap.total_cells, snap.provisioned
        );
        for c in &snap.cells {
            let marker = if c.parked {
                "⛔"
            } else {
                match (c.provisioned, c.healthy, c.pending_since.is_some()) {
                    (false, _, _) => "·",
                    (true, Some(true), _) => "✔",
                    (true, Some(false), _) => "✗",
                    (true, None, true) => "…",
                    (true, None, false) => " ",
                }
            };
            let extra = match (&c.cluster_id, &c.instance_name) {
                (Some(cid), Some(name)) => format!("  cluster={cid}  incus={name}"),
                _ => String::new(),
            };
            let detail = c
                .detail
                .as_deref()
                .map(|d| format!("  [{d}]"))
                .unwrap_or_default();
            let fail = if c.deploy_failures > 0 {
                format!("  fail={}", c.deploy_failures)
            } else {
                String::new()
            };
            println!("  {marker} {}{}{}{}", c.key, extra, fail, detail);
        }
    }
    Ok(())
}

async fn cmd_provision(cfg: &RunnerConfig) -> Result<()> {
    let cli = api::Cli::new(&cfg.api.bind, cfg.api.port);
    let ack = cli.provision().await.context("dialing runner daemon")?;
    println!("provision: ok={}", ack.ok);
    Ok(())
}

async fn cmd_teardown(cfg: &RunnerConfig, yes: bool) -> Result<()> {
    if !yes {
        eprintln!(
            "teardown will delete every provisioned cell. Re-run with --yes to confirm."
        );
        anyhow::bail!("teardown not confirmed");
    }
    let cli = api::Cli::new(&cfg.api.bind, cfg.api.port);
    let ack = cli.teardown().await.context("dialing runner daemon")?;
    println!("teardown: ok={}", ack.ok);
    Ok(())
}

async fn cmd_reprovision(cfg: &RunnerConfig, key: Option<&str>) -> Result<()> {
    let cli = api::Cli::new(&cfg.api.bind, cfg.api.port);
    let ack = cli
        .reprovision(key)
        .await
        .context("dialing runner daemon")?;
    println!(
        "reprovision: ok={} key={}",
        ack.ok,
        ack.detail.as_deref().unwrap_or("<none>")
    );
    Ok(())
}

fn cmd_matrix(cfg: &RunnerConfig, json: bool) -> Result<()> {
    let cells = matrix::generate(&cfg.matrix);
    if json {
        for c in cells {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "key": c.key,
                    "config": c.config,
                }))?
            );
        }
    } else {
        println!("{} cells:", cells.len());
        for c in cells {
            println!("  {}", c.key);
        }
    }
    Ok(())
}
