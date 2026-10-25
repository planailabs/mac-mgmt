//! Subcommands for whatever provisions machines into this server from the
//! outside — plan-ai-cloud's node containers, mainly — run on the server's own
//! host. They read and write the database directly, so the caller needs a
//! shell here rather than an API token, and every answer is JSON on stdout.

use clap::Subcommand;
use serde::Serialize;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum NodeCommand {
    /// Mint a sync token for a cluster and print the cloud-init that enrols a
    /// fresh machine with it, as `{"cloud_init": "..."}`.
    CloudInit {
        #[arg(long)]
        cluster: Uuid,
        /// URL the daemon dials: this server's API as the machine can reach it.
        #[arg(long)]
        server_url: String,
        /// OpenSSH PEM of a pregenerated Ed25519 host key, so the caller knows
        /// the instance_id the daemon will report before it boots.
        #[arg(long)]
        host_key_file: Option<std::path::PathBuf>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long, default_value = "x86_64-linux")]
        system: String,
    },
    /// List clusters as `[{"id", "name"}]`.
    Clusters,
    /// List a cluster's heartbeat-reporting machines.
    Machines {
        #[arg(long)]
        cluster: Uuid,
    },
    /// Drop a machine that is gone for good, so it stops showing as stale.
    ForgetMachine {
        #[arg(long)]
        cluster: Uuid,
        #[arg(long)]
        instance_id: String,
    },
}

#[derive(Serialize, sqlx::FromRow)]
struct Cluster {
    id: Uuid,
    name: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct Machine {
    instance_id: String,
    hostname: Option<String>,
    version: String,
    reported_at: chrono::DateTime<chrono::Utc>,
}

pub async fn run(database_url: &str, cmd: &NodeCommand) -> anyhow::Result<()> {
    use anyhow::Context;

    let pool = crate::db::connect(database_url).await;
    let out = match cmd {
        NodeCommand::CloudInit {
            cluster,
            server_url,
            host_key_file,
            label,
            system,
        } => {
            let host_key = host_key_file
                .as_ref()
                .map(std::fs::read_to_string)
                .transpose()
                .context("reading the host key")?;
            let version = crate::api::routes::resolve_daemon_version(&pool, *cluster)
                .await?
                .context(
                    "no daemon version available (no rollout, no pinned_version, no daemon_versions rows)",
                )?;
            let label = label.clone().unwrap_or_else(|| {
                format!(
                    "cloud-init-cli-{}",
                    chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
                )
            });
            let (cloud_init, _token) = crate::api::routes::mint_cloud_init(
                &pool,
                *cluster,
                server_url,
                &version,
                system,
                &label,
                host_key.as_deref(),
            )
            .await?;
            serde_json::json!({ "cloud_init": cloud_init })
        }
        NodeCommand::Clusters => {
            let rows: Vec<Cluster> = sqlx::query_as("SELECT id, name FROM clusters ORDER BY name")
                .fetch_all(&pool)
                .await?;
            serde_json::to_value(rows)?
        }
        NodeCommand::Machines { cluster } => {
            let rows: Vec<Machine> = sqlx::query_as(
                "SELECT instance_id, hostname, version, reported_at \
                 FROM daemon_heartbeats WHERE cluster_id = $1 ORDER BY reported_at DESC",
            )
            .bind(cluster)
            .fetch_all(&pool)
            .await?;
            serde_json::to_value(rows)?
        }
        NodeCommand::ForgetMachine {
            cluster,
            instance_id,
        } => {
            let forgotten = sqlx::query(
                "DELETE FROM daemon_heartbeats WHERE cluster_id = $1 AND instance_id = $2",
            )
            .bind(cluster)
            .bind(instance_id)
            .execute(&pool)
            .await?
            .rows_affected();
            serde_json::json!({ "forgotten": forgotten })
        }
    };
    println!("{out}");
    Ok(())
}
