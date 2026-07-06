//! mmrc — unified CLI for causing things to happen on a mac-mgmt cluster, in
//! prod (mmr) or the antithesis (fresh incus) environment.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use antithesis_workloads::env::{Ctx, Env, NodeKind};
use antithesis_workloads::rng::Rng;
use antithesis_workloads::workloads::cluster::{ClusterServices, ensure_cluster, ensure_nodes};
use antithesis_workloads::{goals, registry};
use clap::{Parser, Subcommand};
use mmr_causality::api::{CreateRunRequest, ServerSpec};
use mmr_causality::config::Config;
use mmr_causality::emulator::{self, EmulatorOptions};
use mmr_causality::envs::{AntithesisEnv, MmrcdClient, ProdEnv};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "mmrc", about = "cause things to happen on a mac-mgmt cluster")]
struct Cli {
    /// Environment: prod | antithesis. Overrides MMRC_ENV / config default_env.
    #[arg(long, global = true)]
    env: Option<String>,
    #[arg(long, global = true)]
    config: Option<std::path::PathBuf>,
    /// Cluster id override (else resolved by name).
    #[arg(long, global = true)]
    cluster_id: Option<Uuid>,
    /// Antithesis: host-reachable server URL of a running `up` cluster.
    #[arg(long, global = true)]
    server_url: Option<String>,
    /// Antithesis: host-reachable relay URL of a running `up` cluster.
    #[arg(long, global = true)]
    relay_url: Option<String>,
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Ensure a cluster exists with selected managed services.
    Cluster {
        #[command(subcommand)]
        cmd: ClusterCmd,
    },
    /// Node operations.
    Node {
        #[command(subcommand)]
        cmd: NodeCmd,
    },
    /// Relay tunnel operations (mapped to workloads).
    Relay { action: String },
    /// Push an SSE event.
    Sse {
        #[command(subcommand)]
        cmd: SseCmd,
    },
    /// Skills operations (mapped to workloads).
    Skills { action: String },
    /// Memvault operations (mapped to workloads).
    Memvault { action: String },
    /// Check eventual-consistency goals.
    Goals {
        #[arg(long, default_value = "all")]
        goal: String,
    },
    /// Show resolved CI image tag + pipeline status (antithesis).
    Images {
        #[arg(long)]
        git_ref: Option<String>,
    },
    /// Bring up a persistent antithesis cluster.
    Up {
        #[arg(long, default_value_t = 2)]
        nodes: usize,
        #[arg(long)]
        git_ref: Option<String>,
    },
    /// Tear down a run (antithesis).
    Down {
        #[arg(long)]
        run_id: Option<String>,
    },
    /// Run the emulator (antithesis).
    Emulator {
        #[command(subcommand)]
        cmd: EmulatorCmd,
    },
}

#[derive(Subcommand)]
enum ClusterCmd {
    Ensure {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        ollama: bool,
        #[arg(long)]
        openclaw: bool,
        #[arg(long)]
        memvault: bool,
    },
}

#[derive(Subcommand)]
enum NodeCmd {
    Ensure {
        #[arg(short = 'n', long, default_value_t = 1)]
        count: usize,
        #[arg(long)]
        chaos: bool,
    },
    Heartbeat,
    Probes,
}

#[derive(Subcommand)]
enum SseCmd {
    Push {
        #[arg(long)]
        event: String,
        #[arg(long)]
        instance: Option<String>,
    },
}

#[derive(Subcommand)]
enum EmulatorCmd {
    Run {
        #[arg(long)]
        rounds: Option<usize>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        workloads_per_round: Option<usize>,
        #[arg(long)]
        nodes: Option<usize>,
        #[arg(long)]
        keep_on_failure: bool,
        #[arg(long)]
        git_ref: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // reqwest uses rustls-no-provider; install the ring crypto provider once.
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mmrc=info,antithesis_workloads=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let cfg = Config::load(cli.config.as_deref())?;
    let env_name = cfg.resolve_env_name(cli.env.as_deref());

    match &cli.cmd {
        Command::Images { git_ref } => images(&cfg, git_ref.clone()).await,
        Command::Up { nodes, git_ref } => up(&cfg, *nodes, git_ref.clone()).await,
        Command::Down { run_id } => down(&cfg, run_id.clone()).await,
        Command::Emulator {
            cmd: EmulatorCmd::Run {
                rounds,
                seed,
                workloads_per_round,
                nodes,
                keep_on_failure,
                git_ref,
            },
        } => {
            let acfg = cfg
                .env
                .antithesis
                .clone()
                .context("emulator needs [env.antithesis] config")?;
            let opts = EmulatorOptions {
                seed: seed.unwrap_or_else(default_seed),
                rounds: rounds.unwrap_or(cfg.emulator.rounds),
                workloads_per_round: workloads_per_round.unwrap_or(cfg.emulator.workloads_per_round),
                nodes: nodes.unwrap_or(cfg.emulator.nodes),
                keep_on_failure: *keep_on_failure,
                git_ref: git_ref.clone(),
            };
            let failures = emulator::run(&acfg, &cfg.emulator, &opts).await?;
            if failures > 0 {
                std::process::exit(1);
            }
            Ok(())
        }
        other => {
            // Commands that operate on a live cluster.
            let (env, cluster_name) = build_env(&cfg, &env_name, &cli).await?;
            run_cluster_command(env, cluster_name, &cli, other).await
        }
    }
}

/// Seed from MMRC_SEED or a fixed default (deterministic unless overridden;
/// Math.random/time are avoided so runs are reproducible by default).
fn default_seed() -> u64 {
    std::env::var("MMRC_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x5EED_1234_ABCD_0001)
}

async fn build_env(cfg: &Config, env_name: &str, cli: &Cli) -> Result<(Arc<dyn Env>, String)> {
    match env_name {
        "prod" => {
            let pc = cfg.env.prod.clone().context("[env.prod] config required")?;
            let name = pc.cluster_name.clone();
            Ok((Arc::new(ProdEnv::new(&pc)?), name))
        }
        "antithesis" => {
            let ac = cfg
                .env
                .antithesis
                .clone()
                .context("[env.antithesis] config required")?;
            let server = cli
                .server_url
                .clone()
                .context("antithesis single commands need --server-url (from `mmrc up`)")?;
            let relay = cli
                .relay_url
                .clone()
                .context("antithesis single commands need --relay-url (from `mmrc up`)")?;
            let name = ac.cluster_name.clone();
            let env = AntithesisEnv::new(&ac, "adhoc".into(), &server, &relay, "")?;
            Ok((Arc::new(env), name))
        }
        other => bail!("unknown env {other:?}"),
    }
}

async fn build_ctx(env: Arc<dyn Env>, cluster_name: &str, cli: &Cli) -> Result<Ctx> {
    let cluster_id = match cli.cluster_id {
        Some(id) => id,
        None => env.mgmt().ensure_cluster(cluster_name).await?,
    };
    let mut ctx = Ctx::new(env.clone(), cluster_id);
    // Populate known instances from the current machine list.
    if let Ok(machines) = env.mgmt().list_cluster_machines(cluster_id).await {
        ctx.instances = machines.into_iter().map(|m| m.instance_id).collect();
    }
    Ok(ctx)
}

async fn run_cluster_command(
    env: Arc<dyn Env>,
    cluster_name: String,
    cli: &Cli,
    cmd: &Command,
) -> Result<()> {
    let mut rng = Rng::new(default_seed());
    match cmd {
        Command::Cluster {
            cmd: ClusterCmd::Ensure {
                name,
                ollama,
                openclaw,
                memvault,
            },
        } => {
            let cname = name.clone().unwrap_or(cluster_name);
            let id = ensure_cluster(
                env.as_ref(),
                ClusterServices {
                    ollama: *ollama,
                    openclaw: *openclaw,
                    memvault: *memvault,
                },
            )
            .await?;
            println!("cluster {cname} ready: {id}");
            Ok(())
        }
        Command::Node {
            cmd: NodeCmd::Ensure { count, chaos },
        } => {
            let mut ctx = build_ctx(env, &cluster_name, cli).await?;
            let kind = if *chaos { NodeKind::Chaos } else { NodeKind::Fleet };
            let ids = ensure_nodes(&mut ctx, *count, kind).await?;
            println!("{} node(s) up: {ids:?}", ids.len());
            Ok(())
        }
        Command::Node { cmd: NodeCmd::Heartbeat } => {
            let ctx = build_ctx(env, &cluster_name, cli).await?;
            goals::heartbeats_fresh(&ctx, &ctx.instances).await?;
            println!("heartbeats fresh for {} instance(s)", ctx.instances.len());
            Ok(())
        }
        Command::Node { cmd: NodeCmd::Probes } => {
            let ctx = build_ctx(env, &cluster_name, cli).await?;
            ctx.mgmt().push_event(ctx.cluster_id, "request_assessment", None).await?;
            goals::all_probes_healthy(&ctx).await?;
            println!("all probes healthy");
            Ok(())
        }
        Command::Sse { cmd: SseCmd::Push { event, instance } } => {
            let ctx = build_ctx(env, &cluster_name, cli).await?;
            let r = ctx
                .mgmt()
                .push_event(ctx.cluster_id, event, instance.as_deref())
                .await?;
            println!("pushed {event}: ok={} receivers={}", r.ok, r.receivers);
            Ok(())
        }
        Command::Goals { goal } => {
            let ctx = build_ctx(env, &cluster_name, cli).await?;
            check_goal(&ctx, goal).await
        }
        Command::Relay { action } => run_named(env, &cluster_name, cli, &mut rng, &format!("relay-{action}")).await,
        Command::Skills { action } => {
            let name = match action.as_str() {
                "assign" | "assert" => "skills-assign-assert",
                "remove" => "skills-remove",
                other => bail!("unknown skills action {other}"),
            };
            run_named(env, &cluster_name, cli, &mut rng, name).await
        }
        Command::Memvault { action } => {
            let name = match action.as_str() {
                "doc" => "memvault-doc",
                "graph" => "memvault-graph",
                other => bail!("unknown memvault action {other}"),
            };
            run_named(env, &cluster_name, cli, &mut rng, name).await
        }
        _ => unreachable!("handled in main"),
    }
}

async fn check_goal(ctx: &Ctx, goal: &str) -> Result<()> {
    match goal {
        "services" => goals::all_services_healthy(ctx).await?,
        "probes" => goals::all_probes_healthy(ctx).await?,
        "heartbeats" => goals::heartbeats_fresh(ctx, &ctx.instances).await?,
        "all" => {
            goals::heartbeats_fresh(ctx, &ctx.instances).await?;
            goals::all_services_healthy(ctx).await?;
            goals::all_probes_healthy(ctx).await?;
        }
        other => bail!("unknown goal {other}"),
    }
    println!("goal '{goal}' holds");
    Ok(())
}

async fn run_named(
    env: Arc<dyn Env>,
    cluster_name: &str,
    cli: &Cli,
    rng: &mut Rng,
    name: &str,
) -> Result<()> {
    let ctx = build_ctx(env, cluster_name, cli).await?;
    let all = registry();
    let wl = all
        .iter()
        .find(|w| w.name() == name)
        .with_context(|| format!("no workload named {name}"))?;
    let outcome = wl.run(&ctx, rng).await?;
    println!("{name}: {outcome:?}");
    Ok(())
}

async fn images(cfg: &Config, git_ref: Option<String>) -> Result<()> {
    let ac = cfg.env.antithesis.clone().context("[env.antithesis] config required")?;
    let client = reqwest::Client::new();
    let mut req = client
        .get(format!("{}/images", ac.mmrcd_url.trim_end_matches('/')))
        .bearer_auth(&ac.mmrcd_token);
    if let Some(r) = git_ref {
        req = req.query(&[("git_ref", r)]);
    }
    let resp = req.send().await.context("querying mmrcd /images")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("mmrcd /images returned {status}: {body}");
    }
    println!("{body}");
    Ok(())
}

async fn up(cfg: &Config, nodes: usize, git_ref: Option<String>) -> Result<()> {
    let ac = cfg.env.antithesis.clone().context("[env.antithesis] config required")?;
    let client = MmrcdClient::new(&ac.mmrcd_url, &ac.mmrcd_token);
    let run = client
        .create_run(&CreateRunRequest {
            git_ref,
            servers: vec![ServerSpec {
                name: "test-mac-mgmt-server".into(),
                mode: "monolith".into(),
                relays: 1,
            }],
            nodes,
        })
        .await?;
    println!("run {} up (image {})", run.run_id, run.image_tag);
    let url = |addr: &str| {
        if addr.starts_with("http") {
            addr.to_string()
        } else {
            format!("https://{addr}")
        }
    };
    let primary = run.servers.first();
    for s in &run.servers {
        println!("  server {} @ {}", s.name, url(&s.addr));
        for r in &s.relays {
            println!("    relay {} @ {}", r.name, url(&r.addr));
        }
    }
    if let Some(s) = primary.and_then(|s| s.relays.first().map(|r| (s, r))) {
        println!(
            "Use: mmrc --env antithesis --server-url {} --relay-url {} <command>",
            url(&s.0.addr),
            url(&s.1.addr)
        );
    }
    Ok(())
}

async fn down(cfg: &Config, run_id: Option<String>) -> Result<()> {
    let ac = cfg.env.antithesis.clone().context("[env.antithesis] config required")?;
    let client = MmrcdClient::new(&ac.mmrcd_url, &ac.mmrcd_token);
    let run_id = run_id.context("--run-id required")?;
    client.delete_run(&run_id).await?;
    println!("run {run_id} torn down");
    Ok(())
}
