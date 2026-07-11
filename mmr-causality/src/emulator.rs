//! The emulator: per round, spin up a fresh incus cluster via mmrcd, seed it,
//! run randomly-picked workloads, verify EC goals, and tear down. Reproducible
//! from a printed seed (sim-tests style).

use std::sync::Arc;

use antithesis_workloads::env::{Ctx, Env, NodeKind};
use antithesis_workloads::rng::Rng;
use antithesis_workloads::workloads::cluster::{ClusterServices, ensure_cluster, ensure_nodes};
use antithesis_workloads::{assert, goals, registry};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::api::{CreateRunRequest, ServerSpec};
use crate::config::{AntithesisConfig, EmulatorConfig};
use crate::envs::{AntithesisEnv, MmrcdClient};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum RoundStage {
    Pending,
    RunCreated { run_id: String },
    ClusterSeeded { cluster_id: String },
    NodesUp { instances: Vec<String> },
    Running { executed: Vec<String> },
    Verified,
    TornDown,
    Failed { at: String, error: String },
}

pub struct EmulatorOptions {
    pub seed: u64,
    pub rounds: usize,
    pub workloads_per_round: usize,
    pub nodes: usize,
    pub keep_on_failure: bool,
    pub git_ref: Option<String>,
}

/// Run the emulator. Returns the number of failed rounds.
pub async fn run(
    acfg: &AntithesisConfig,
    ecfg: &EmulatorConfig,
    opts: &EmulatorOptions,
) -> Result<usize> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║  mmrc emulator — seed={:016x}          ", opts.seed);
    println!(
        "║  Reproduce: mmrc emulator run --seed {} --rounds 1",
        opts.seed
    );
    println!("╚══════════════════════════════════════════════╝");

    assert::init(Some(&ecfg.run_dir.join("assertions.jsonl")));

    let mut failures = 0;
    for round in 0..opts.rounds {
        let round_seed = Rng::derive(opts.seed, round as u64);
        println!("── round {round} (seed={round_seed:016x}) ──");
        match run_round(acfg, ecfg, opts, round_seed).await {
            Ok(()) => println!("   round {round}: PASS"),
            Err(e) => {
                failures += 1;
                println!("   round {round}: FAIL — {e:#}");
            }
        }
    }
    println!(
        "── summary: {}/{} rounds passed ──",
        opts.rounds - failures,
        opts.rounds
    );
    Ok(failures)
}

async fn run_round(
    acfg: &AntithesisConfig,
    ecfg: &EmulatorConfig,
    opts: &EmulatorOptions,
    seed: u64,
) -> Result<()> {
    let mut rng = Rng::new(seed);
    let mmrcd = MmrcdClient::new(&acfg.mmrcd_url, &acfg.mmrcd_token);

    // 1. Fresh run: server + relay from CI images.
    let run = mmrcd
        .create_run(&CreateRunRequest {
            git_ref: opts.git_ref.clone(),
            servers: vec![ServerSpec {
                name: "test-mac-mgmt-server".into(),
                mode: "monolith".into(),
                relays: 1,
            }],
            nodes: 0,
        })
        .await
        .context("creating mmrcd run")?;
    let run_id = run.run_id.clone();
    persist_stage(
        ecfg,
        &run_id,
        &RoundStage::RunCreated {
            run_id: run_id.clone(),
        },
    );

    // Tear down at the end unless we're keeping a failed run.
    let result = run_round_inner(acfg, ecfg, opts, &mut rng, &run).await;
    match &result {
        Ok(()) => {
            mmrcd.delete_run(&run_id).await.ok();
            persist_stage(ecfg, &run_id, &RoundStage::TornDown);
        }
        Err(e) => {
            persist_stage(
                ecfg,
                &run_id,
                &RoundStage::Failed {
                    at: "round".into(),
                    error: e.to_string(),
                },
            );
            if !opts.keep_on_failure {
                mmrcd.delete_run(&run_id).await.ok();
            } else {
                println!("   keeping run {run_id} for inspection");
            }
        }
    }
    result
}

async fn run_round_inner(
    acfg: &AntithesisConfig,
    ecfg: &EmulatorConfig,
    opts: &EmulatorOptions,
    rng: &mut Rng,
    run: &crate::api::RunInfo,
) -> Result<()> {
    // Resolve host-reachable server/relay addresses from the run.
    let server = run.servers.first().context("run has no server")?;
    let relay = server.relays.first().context("server has no relay")?;
    let server_url = normalize_url(&server.addr);
    let relay_url = normalize_url(&relay.addr);

    // 2. Ensure cluster (with all managed services) + mint a proxy token.
    let env0 = AntithesisEnv::new(acfg, run.run_id.clone(), &server_url, &relay_url, "")?;
    let cluster_id = ensure_cluster(
        &env0,
        ClusterServices {
            ollama: true,
            openclaw: true,
            memvault: true,
        },
    )
    .await
    .context("ensure_cluster")?;
    persist_stage(
        ecfg,
        &run.run_id,
        &RoundStage::ClusterSeeded {
            cluster_id: cluster_id.to_string(),
        },
    );

    // Rebuild the env with a real proxy token now that the cluster exists.
    let proxy_token = env0
        .mgmt()
        .create_proxy_token(cluster_id, &[])
        .await
        .unwrap_or_default();
    let env = Arc::new(AntithesisEnv::new(
        acfg,
        run.run_id.clone(),
        &server_url,
        &relay_url,
        &proxy_token,
    )?);

    let mut ctx = Ctx::new(env.clone(), cluster_id);
    ctx.timeouts = env.timeouts();

    // 3. Ensure fleet nodes and mark setup complete.
    let instances = ensure_nodes(&mut ctx, opts.nodes, NodeKind::Fleet)
        .await
        .context("ensure_nodes")?;
    assert::setup_complete(&serde_json::json!({
        "run_id": run.run_id,
        "cluster_id": cluster_id.to_string(),
        "instances": instances,
    }));
    persist_stage(ecfg, &run.run_id, &RoundStage::NodesUp { instances });

    // 4. Pick K workloads at random and run them.
    let all = registry();
    let mut executed = Vec::new();
    for _ in 0..opts.workloads_per_round {
        let idx = rng.below(all.len());
        let wl = &all[idx];
        let name = wl.name();
        match wl.run(&ctx, rng).await {
            Ok(outcome) => {
                tracing::info!("workload {name}: {outcome:?}");
                executed.push(format!("{name}:{outcome:?}"));
            }
            Err(e) => {
                executed.push(format!("{name}:ERR"));
                persist_stage(ecfg, &run.run_id, &RoundStage::Running { executed });
                return Err(e).with_context(|| format!("workload {name}"));
            }
        }
    }
    persist_stage(ecfg, &run.run_id, &RoundStage::Running { executed });

    // 5. Final EC sweep.
    goals::all_services_healthy(&ctx)
        .await
        .context("final: services")?;
    goals::heartbeats_fresh(&ctx, &ctx.instances)
        .await
        .context("final: heartbeats")?;
    persist_stage(ecfg, &run.run_id, &RoundStage::Verified);
    Ok(())
}

fn normalize_url(addr: &str) -> String {
    if addr.starts_with("http://") || addr.starts_with("https://") {
        addr.to_string()
    } else {
        format!("https://{addr}")
    }
}

fn persist_stage(ecfg: &EmulatorConfig, run_id: &str, stage: &RoundStage) {
    let dir = ecfg.run_dir.join(run_id);
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(
            dir.join("stage.json"),
            serde_json::to_vec_pretty(stage).unwrap_or_default(),
        );
    }
    tracing::info!(run_id, ?stage, "stage");
}
