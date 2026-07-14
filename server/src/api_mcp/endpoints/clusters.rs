//! Cluster endpoints: list/get/create/rename/delete plus cluster-scoped
//! settings (nixpkgs pin, daemon version pin, cloud-init, secrets, manual
//! packages, healer settings) and permission probes.

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{Cluster, ClusterConfig};

#[cfg(feature = "server")]
use super::internal;
#[cfg(feature = "server")]
use crate::api_mcp::access;
#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterListInput {}

/// A cluster with its owning organizations, as shown in the cluster list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterRow {
    pub id: String,
    pub name: String,
    pub org_names: Vec<String>,
    pub pinned_version: Option<String>,
    pub nixpkgs_commit: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Handlers ────────────────────────────────────────────────────────────

/// was: list_clusters() in web/components/cluster_list.rs
#[api_mcp_dioxus_server(server = "list_clusters")]
pub async fn cluster_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    _input: ClusterListInput,
) -> Result<Vec<ClusterRow>, ApiError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
        org_names: Vec<String>,
        pinned_version: Option<String>,
        nixpkgs_commit: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    const BASE: &str = "SELECT c.id, c.name, \
         COALESCE(array_agg(DISTINCT o.name) FILTER (WHERE o.name IS NOT NULL), '{}') AS org_names, \
         c.pinned_version, c.nixpkgs_commit, c.created_at \
         FROM clusters c \
         LEFT JOIN organization_clusters oc ON oc.cluster_id = c.id \
         LEFT JOIN organizations o ON o.id = oc.organization_id";

    let rows = if let Some(ids) = access::accessible_cluster_ids(pool, p).await? {
        sqlx::query_as::<_, Row>(&format!(
            "{BASE} WHERE c.id = ANY($1) GROUP BY c.id ORDER BY c.name"
        ))
        .bind(&ids)
        .fetch_all(pool)
        .await
        .map_err(internal)?
    } else {
        sqlx::query_as::<_, Row>(&format!("{BASE} GROUP BY c.id ORDER BY c.name"))
            .fetch_all(pool)
            .await
            .map_err(internal)?
    };

    Ok(rows
        .into_iter()
        .map(|r| ClusterRow {
            id: r.id.to_string(),
            name: r.name,
            org_names: r.org_names,
            pinned_version: r.pinned_version,
            nixpkgs_commit: r.nixpkgs_commit,
            created_at: r.created_at,
        })
        .collect())
}

// ── CRUD DTOs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCreateInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterUpdateInput {
    pub id: Uuid,
    /// New display name for the cluster.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterDeleteInput {
    pub id: Uuid,
}

// ── CRUD handlers ───────────────────────────────────────────────────────

/// was: get_cluster() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "get_cluster")]
pub async fn cluster_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterGetInput,
) -> Result<Cluster, ApiError> {
    access::require_cluster_read(pool, p, input.id).await?;
    let cluster = sqlx::query_as::<_, Cluster>("SELECT * FROM clusters WHERE id = $1")
        .bind(input.id)
        .fetch_optional(pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::not_found("cluster not found"))?;
    Ok(cluster)
}

/// was: create_cluster() in web/components/cluster_form.rs
#[api_mcp_dioxus_server(server = "create_cluster")]
pub async fn cluster_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCreateInput,
) -> Result<Cluster, ApiError> {
    p.require_admin()?;
    let cluster =
        sqlx::query_as::<_, Cluster>("INSERT INTO clusters (name) VALUES ($1) RETURNING *")
            .bind(&input.name)
            .fetch_one(pool)
            .await
            .map_err(internal)?;
    Ok(cluster)
}

/// was: rename_cluster() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "rename_cluster")]
pub async fn cluster_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterUpdateInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("UPDATE clusters SET name = $1 WHERE id = $2")
        .bind(&input.name)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

/// was: delete_cluster() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "delete_cluster")]
pub async fn cluster_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterDeleteInput,
) -> Result<(), ApiError> {
    p.require_admin()?;
    sqlx::query("DELETE FROM clusters WHERE id = $1")
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;
    Ok(())
}

// ── Permission probes ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCanWriteInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClusterCanAdminInput {
    pub id: Uuid,
}

/// was: can_write_cluster() in web/components/cluster_detail.rs (and dupes)
#[api_mcp_dioxus_server(server = "can_write_cluster")]
pub async fn cluster_can_write(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCanWriteInput,
) -> Result<bool, ApiError> {
    match access::writable_cluster_ids(pool, p).await? {
        Some(ids) => Ok(ids.contains(&input.id)),
        None => Ok(true),
    }
}

/// was: can_admin_cluster() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "can_admin_cluster")]
pub async fn cluster_can_admin(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ClusterCanAdminInput,
) -> Result<bool, ApiError> {
    access::is_cluster_org_admin(pool, p, input.id).await
}

// ── Version / nixpkgs pinning ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PinnedRolloutInput {
    /// Daemon version to look up the most recent rollout for.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetPinnedVersionInput {
    pub id: Uuid,
    /// Daemon version to pin; empty/whitespace clears the pin.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetNixpkgsInput {
    pub id: Uuid,
    /// nixpkgs commit sha (7-40 hex chars); empty/whitespace clears the pin.
    pub commit: String,
}

/// was: get_pinned_rollout() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "get_pinned_rollout")]
pub async fn cluster_pinned_rollout(
    pool: &sqlx::PgPool,
    _p: &Principal,
    input: PinnedRolloutInput,
) -> Result<Option<String>, ApiError> {
    let rollout_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM rollouts \
         WHERE target_version = $1 \
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(&input.version)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    Ok(rollout_id.map(|id| id.to_string()))
}

/// was: set_pinned_version() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "set_pinned_version")]
pub async fn cluster_set_pinned_version(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SetPinnedVersionInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;
    let ver = input.version.trim().to_string();
    if ver.is_empty() {
        sqlx::query("UPDATE clusters SET pinned_version = NULL WHERE id = $1")
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(internal)?;
    } else {
        sqlx::query("UPDATE clusters SET pinned_version = $1 WHERE id = $2")
            .bind(&ver)
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(internal)?;
    }
    Ok(())
}

/// was: set_nixpkgs_commit() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "set_nixpkgs_commit")]
pub async fn cluster_set_nixpkgs(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SetNixpkgsInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;
    let c = input.commit.trim().to_string();
    if c.is_empty() {
        sqlx::query("UPDATE clusters SET nixpkgs_commit = NULL WHERE id = $1")
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(internal)?;
    } else {
        let valid = (7..=40).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err(ApiError::bad_request("commit must be 7-40 hex chars"));
        }

        let current_commit: Option<String> =
            sqlx::query_scalar("SELECT nixpkgs_commit FROM clusters WHERE id = $1")
                .bind(input.id)
                .fetch_one(pool)
                .await
                .map_err(internal)?;
        let mut shas: std::collections::HashSet<String> = [c.clone()].into_iter().collect();
        if let Some(ref current) = current_commit {
            shas.insert(current.clone());
        }
        let counts = crate::commit_count::nixpkgs_commit_counts(&shas).await;
        let new_count = counts
            .get(&c)
            .ok_or_else(|| ApiError::bad_request(format!("unknown nixpkgs commit {c}")))?;
        if let Some(ref current) = current_commit {
            let cur_count = counts.get(current).ok_or_else(|| {
                ApiError::internal(format!("cannot resolve commit count for current {current}"))
            })?;
            if new_count < cur_count {
                let short_new: String = c.chars().take(12).collect();
                let short_cur: String = current.chars().take(12).collect();
                return Err(ApiError::bad_request(format!(
                    "nixpkgs {short_new} (#{new_count}) is older than current {short_cur} (#{cur_count}); use rollback to downgrade"
                )));
            }
        }

        sqlx::query("UPDATE clusters SET nixpkgs_commit = $1 WHERE id = $2")
            .bind(&c)
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(internal)?;
    }
    crate::api::push::notify_global(input.id, crate::api::push::PushMessage::SyncNixpkgs).await;
    Ok(())
}

// ── Rollouts / cloud-init ───────────────────────────────────────────────

/// A rollout currently targeting the cluster (status rolling or paused).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ActiveRolloutEntry {
    pub id: String,
    pub target_version: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ActiveRolloutsInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CloudInitInput {
    pub id: Uuid,
}

/// was: get_active_rollouts() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "get_active_rollouts")]
pub async fn cluster_active_rollouts(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ActiveRolloutsInput,
) -> Result<Vec<ActiveRolloutEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        target_version: Option<String>,
        status: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT DISTINCT r.id, r.target_version, r.status FROM rollouts r \
         JOIN rollout_stages rs ON rs.rollout_id = r.id \
         JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id \
         WHERE rgm.cluster_id = $1 AND r.status IN ('rolling', 'paused') \
         ORDER BY r.target_version DESC",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| ActiveRolloutEntry {
            id: r.id.to_string(),
            target_version: r.target_version,
            status: r.status,
        })
        .collect())
}

/// was: get_cloud_init() in web/components/cluster_detail.rs
#[api_mcp_dioxus_server(server = "get_cloud_init")]
pub async fn cluster_cloud_init(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: CloudInitInput,
) -> Result<String, ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;
    let cid = input.id;

    // Same resolution as /api/update, including delivered-rollout stickiness.
    let rollout_version: Option<String> =
        crate::api::routes::active_rollout_for_cluster(pool, cid)
            .await
            .map_err(internal)?
            .and_then(|r| r.target_version);
    let pinned: Option<String> =
        sqlx::query_scalar("SELECT pinned_version FROM clusters WHERE id = $1")
            .bind(cid)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .flatten();
    // Highest semver only — channel versions ("rolling") are opt-in via
    // pin or rollout, and the int[] cast would error on them.
    let latest: Option<String> = sqlx::query_scalar(
        "SELECT version FROM daemon_versions \
         WHERE version ~ '^[0-9]+(\\.[0-9]+)*$' \
         ORDER BY string_to_array(version, '.')::int[] DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    let version = rollout_version.or(pinned).or(latest).ok_or_else(|| {
        ApiError::not_found(
            "no daemon version available (no rollout, no pinned_version, no daemon_versions rows)",
        )
    })?;

    let server_url = crate::config::config()
        .api
        .external_url
        .trim_end_matches('/')
        .to_string();

    use rand::Rng;
    use sha2::{Digest, Sha256};
    let raw_token = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let label = format!(
        "cloud-init-webui-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    );
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind) VALUES ($1, $2, $3, 'sync')",
    )
    .bind(cid)
    .bind(&hash)
    .bind(&label)
    .execute(pool)
    .await
    .map_err(internal)?;

    Ok(crate::api::routes::render_cloud_init(
        &server_url,
        &raw_token,
        &version,
        "x86_64-linux",
        None,
    ))
}

// ── Secrets ─────────────────────────────────────────────────────────────

/// A cluster secret (name + creation time; values are never returned).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecretEntry {
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecretsListInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecretCreateInput {
    pub id: Uuid,
    pub name: String,
    /// Plaintext secret value; stored encrypted at rest.
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecretUpdateInput {
    pub id: Uuid,
    pub name: String,
    /// New plaintext secret value; stored encrypted at rest.
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecretDeleteInput {
    pub id: Uuid,
    pub name: String,
}

#[cfg(feature = "server")]
fn encrypt_secret_value(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit, OsRng};
    use aes_gcm::{AeadCore, Aes256Gcm, Key};

    let cfg = crate::config::config();
    let secrets = cfg
        .secrets
        .as_ref()
        .ok_or_else(|| "secrets not configured".to_string())?;
    let key_b64 = &secrets.encryption_key;
    let key_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key_b64)
        .map_err(|e| format!("invalid encryption key: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("encryption key must be 32 bytes".into());
    }
    let key = *Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| "encryption failed")?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// was: list_secrets() in web/components/cluster_config_page.rs
#[api_mcp_dioxus_server(server = "list_secrets")]
pub async fn cluster_secrets_list(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SecretsListInput,
) -> Result<Vec<SecretEntry>, ApiError> {
    access::require_cluster_read(pool, p, input.id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        name: String,
        created_at: chrono::DateTime<chrono::Utc>,
    }
    let rows = sqlx::query_as::<_, Row>(
        "SELECT name, created_at FROM cluster_secrets WHERE cluster_id = $1 ORDER BY name",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| SecretEntry {
            name: r.name,
            created_at: r.created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

/// was: create_secret() in web/components/cluster_config_page.rs
#[api_mcp_dioxus_server(server = "create_secret")]
pub async fn cluster_secret_create(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SecretCreateInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;

    let encrypted = encrypt_secret_value(input.value.as_bytes())
        .map_err(|e| ApiError::internal(format!("encryption error: {e}")))?;

    let name = &input.name;
    sqlx::query(
        "INSERT INTO cluster_secrets (cluster_id, name, encrypted_value) VALUES ($1, $2, $3)",
    )
    .bind(input.id)
    .bind(name)
    .bind(&encrypted)
    .execute(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") {
            ApiError::conflict(format!("secret '{name}' already exists"))
        } else {
            internal(e)
        }
    })?;

    crate::api::push::notify_global(input.id, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

/// was: update_secret() in web/components/cluster_config_page.rs
#[api_mcp_dioxus_server(server = "update_secret")]
pub async fn cluster_secret_update(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SecretUpdateInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;

    let encrypted = encrypt_secret_value(input.value.as_bytes())
        .map_err(|e| ApiError::internal(format!("encryption error: {e}")))?;

    let result = sqlx::query(
        "UPDATE cluster_secrets SET encrypted_value = $1, updated_at = now() \
         WHERE cluster_id = $2 AND name = $3",
    )
    .bind(&encrypted)
    .bind(input.id)
    .bind(&input.name)
    .execute(pool)
    .await
    .map_err(internal)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("secret not found"));
    }

    crate::api::push::notify_global(input.id, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

/// was: delete_secret() in web/components/cluster_config_page.rs
#[api_mcp_dioxus_server(server = "delete_secret")]
pub async fn cluster_secret_delete(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: SecretDeleteInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;

    let result = sqlx::query("DELETE FROM cluster_secrets WHERE cluster_id = $1 AND name = $2")
        .bind(input.id)
        .bind(&input.name)
        .execute(pool)
        .await
        .map_err(internal)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("secret not found"));
    }

    crate::api::push::notify_global(input.id, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

// ── Packages ────────────────────────────────────────────────────────────

/// A manually added cluster package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageRow {
    pub id: String,
    pub package: String,
}

/// A package with the sources that pull it in (manual, mcp:<slug>, skill:<slug>).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageWithSources {
    pub package: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ManualPackagesInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageAddInput {
    pub id: Uuid,
    /// Nix package attribute name to install on the cluster.
    pub package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackageRemoveInput {
    pub id: Uuid,
    /// Row id of the manual package entry (from packages_manual).
    pub package_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AllPackagesInput {
    pub id: Uuid,
}

/// was: list_manual_packages() in web/components/cluster_packages.rs
#[api_mcp_dioxus_server(server = "list_manual_packages")]
pub async fn cluster_packages_manual(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ManualPackagesInput,
) -> Result<Vec<PackageRow>, ApiError> {
    access::require_cluster_read(pool, p, input.id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        package: String,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, package FROM cluster_packages WHERE cluster_id = $1 ORDER BY package",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| PackageRow {
            id: r.id.to_string(),
            package: r.package,
        })
        .collect())
}

/// was: add_manual_package() in web/components/cluster_packages.rs
#[api_mcp_dioxus_server(server = "add_manual_package")]
pub async fn cluster_package_add(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: PackageAddInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;

    let pkg = input.package.trim().to_string();
    if pkg.is_empty() {
        return Err(ApiError::bad_request("package name is empty"));
    }

    sqlx::query(
        "INSERT INTO cluster_packages (cluster_id, package) VALUES ($1, $2) \
         ON CONFLICT (cluster_id, package) DO NOTHING",
    )
    .bind(input.id)
    .bind(&pkg)
    .execute(pool)
    .await
    .map_err(internal)?;

    crate::api::push::notify_global(input.id, crate::api::push::PushMessage::SyncPackages).await;
    Ok(())
}

/// was: remove_manual_package() in web/components/cluster_packages.rs
#[api_mcp_dioxus_server(server = "remove_manual_package")]
pub async fn cluster_package_remove(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: PackageRemoveInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;

    sqlx::query("DELETE FROM cluster_packages WHERE id = $1 AND cluster_id = $2")
        .bind(input.package_id)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(internal)?;

    crate::api::push::notify_global(input.id, crate::api::push::PushMessage::SyncPackages).await;
    Ok(())
}

/// was: list_all_packages() in web/components/cluster_packages.rs
#[api_mcp_dioxus_server(server = "list_all_packages")]
pub async fn cluster_packages_all(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: AllPackagesInput,
) -> Result<Vec<PackageWithSources>, ApiError> {
    use std::collections::HashMap;
    access::require_cluster_read(pool, p, input.id).await?;

    let mut packages: HashMap<String, Vec<String>> = HashMap::new();

    #[derive(sqlx::FromRow)]
    struct PkgRow {
        slug: String,
        nix_packages: Vec<String>,
    }
    let mcp_rows: Vec<PkgRow> = sqlx::query_as(
        "SELECT ms.slug, ms.nix_packages \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1 AND ms.nix_packages != '{}' \
         UNION ALL \
         SELECT ms.slug, ms.nix_packages \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE cmb.cluster_id = $1 AND ms.nix_packages != '{}'",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for row in &mcp_rows {
        for pkg in &row.nix_packages {
            packages
                .entry(pkg.clone())
                .or_default()
                .push(format!("mcp:{}", row.slug));
        }
    }

    #[derive(sqlx::FromRow)]
    struct SkillRow {
        skill_slug: String,
        nix_packages: Vec<String>,
    }
    let skill_rows: Vec<SkillRow> = sqlx::query_as(
        "SELECT s.slug AS skill_slug, sc.nix_packages \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1 AND sc.nix_packages != '{}' \
         UNION ALL \
         SELECT s.slug AS skill_slug, sc.nix_packages \
         FROM cluster_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cb.cluster_id = $1 AND sc.nix_packages != '{}'",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for row in &skill_rows {
        for pkg in &row.nix_packages {
            packages
                .entry(pkg.clone())
                .or_default()
                .push(format!("skill:{}", row.skill_slug));
        }
    }

    let manual: Vec<String> =
        sqlx::query_scalar("SELECT package FROM cluster_packages WHERE cluster_id = $1")
            .bind(input.id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    for pkg in manual {
        packages.entry(pkg).or_default().push("manual".to_string());
    }

    let mut result: Vec<PackageWithSources> = packages
        .into_iter()
        .map(|(package, mut sources)| {
            sources.sort();
            sources.dedup();
            PackageWithSources { package, sources }
        })
        .collect();
    result.sort_by(|a, b| a.package.cmp(&b.package));

    Ok(result)
}

// ── Healer settings ─────────────────────────────────────────────────────

/// Per-cluster healer override settings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealerSettings {
    pub enabled: bool,
    pub auto_trigger: bool,
    /// "provider:model" key for the auto-trigger model, or "none".
    pub auto_trigger_key: String,
    pub auto_approve: bool,
    /// "provider:model" key for the fix model, or "none".
    pub fix_model_key: String,
}

/// A selectable healer model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealerModelOption {
    pub key: String,
    pub name: String,
    pub provider: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealerSettingsData {
    pub settings: HealerSettings,
    pub models: Vec<HealerModelOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealerSettingsInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HealerSettingsSetInput {
    pub id: Uuid,
    /// Whether the cluster overrides the server-wide healer defaults.
    pub enabled: bool,
    pub auto_trigger: bool,
    pub auto_trigger_provider: Option<String>,
    pub auto_trigger_model: Option<String>,
    pub auto_approve: bool,
    pub fix_provider: Option<String>,
    pub fix_model: Option<String>,
}

/// was: load_settings() in web/components/cluster_healer_settings.rs
#[api_mcp_dioxus_server(server = "load_settings")]
pub async fn cluster_healer_settings_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: HealerSettingsInput,
) -> Result<HealerSettingsData, ApiError> {
    access::require_cluster_read(pool, p, input.id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        enabled: bool,
        auto_trigger: Option<bool>,
        auto_trigger_provider: Option<String>,
        auto_trigger_model: Option<String>,
        auto_approve: Option<bool>,
        fix_provider: Option<String>,
        fix_model: Option<String>,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT enabled, auto_trigger, auto_trigger_provider, auto_trigger_model, \
                auto_approve, fix_provider, fix_model \
         FROM healer_cluster_settings WHERE cluster_id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;

    let settings = row
        .map(|r| HealerSettings {
            enabled: r.enabled,
            auto_trigger: r.auto_trigger.unwrap_or(false),
            auto_trigger_key: match (r.auto_trigger_provider, r.auto_trigger_model) {
                (Some(p), Some(m)) => format!("{p}:{m}"),
                _ => "none".to_string(),
            },
            auto_approve: r.auto_approve.unwrap_or(false),
            fix_model_key: match (r.fix_provider, r.fix_model) {
                (Some(p), Some(m)) => format!("{p}:{m}"),
                _ => "none".to_string(),
            },
        })
        .unwrap_or_default();

    let healer_cfg = &crate::config::config().healer;
    let model_entries = if healer_cfg.models.is_empty() {
        crate::config::default_healer_models()
    } else {
        healer_cfg.models.clone()
    };
    let models = model_entries
        .into_iter()
        .map(|e| HealerModelOption {
            key: format!("{}:{}", e.provider, e.model),
            name: e.display_name(),
            provider: e.provider,
        })
        .collect();

    Ok(HealerSettingsData { settings, models })
}

/// was: save_settings() in web/components/cluster_healer_settings.rs
#[api_mcp_dioxus_server(server = "save_settings")]
pub async fn cluster_healer_settings_set(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: HealerSettingsSetInput,
) -> Result<(), ApiError> {
    access::require_cluster_write(pool, p, input.id).await?;

    sqlx::query(
        "INSERT INTO healer_cluster_settings \
            (cluster_id, enabled, auto_trigger, auto_trigger_provider, auto_trigger_model, \
             auto_approve, fix_provider, fix_model, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now()) \
         ON CONFLICT (cluster_id) DO UPDATE SET \
            enabled = EXCLUDED.enabled, \
            auto_trigger = EXCLUDED.auto_trigger, \
            auto_trigger_provider = EXCLUDED.auto_trigger_provider, \
            auto_trigger_model = EXCLUDED.auto_trigger_model, \
            auto_approve = EXCLUDED.auto_approve, \
            fix_provider = EXCLUDED.fix_provider, \
            fix_model = EXCLUDED.fix_model, \
            updated_at = now()",
    )
    .bind(input.id)
    .bind(input.enabled)
    .bind(input.auto_trigger)
    .bind(&input.auto_trigger_provider)
    .bind(&input.auto_trigger_model)
    .bind(input.auto_approve)
    .bind(&input.fix_provider)
    .bind(&input.fix_model)
    .execute(pool)
    .await
    .map_err(internal)?;
    Ok(())
}

// ── Cluster config ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigGetInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigSaveInput {
    pub id: Uuid,
    /// The full cluster config document as a JSON string; it is migrated to
    /// the current schema and validated before being stored.
    pub config_json: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigSchemaInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigHistoryInput {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigDiffInput {
    /// cluster_configs row id of the left (older) revision.
    pub left_id: Uuid,
    /// cluster_configs row id of the right (newer) revision.
    pub right_id: Uuid,
}

/// A stored cluster config revision (id + creation time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConfigVersion {
    pub id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// One line of a config diff: tag is "equal", "insert" or "delete".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiffLine {
    pub tag: String,
    pub content: String,
}

/// was: get_current_config() in web/components/config_editor_panel.rs
#[api_mcp_dioxus_server(server = "get_current_config")]
pub async fn cluster_config_get(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ConfigGetInput,
) -> Result<Option<ClusterConfig>, ApiError> {
    access::require_cluster_read(pool, p, input.id).await?;
    let config = sqlx::query_as::<_, ClusterConfig>(
        "SELECT * FROM cluster_configs WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?;
    Ok(config.map(|mut c| {
        mac_mgmt_common::config_migrate::migrate(&mut c.config_json);
        c
    }))
}

/// was: save_config() in web/components/config_editor_panel.rs
#[api_mcp_dioxus_server(server = "save_config")]
pub async fn cluster_config_save(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ConfigSaveInput,
) -> Result<(), ApiError> {
    let mut json: serde_json::Value = serde_json::from_str(&input.config_json)
        .map_err(|e| ApiError::bad_request(format!("invalid JSON: {e}")))?;
    mac_mgmt_common::config_migrate::migrate(&mut json);
    // Validate
    let _: mac_mgmt_common::ClusterConfig = serde_json::from_value(json.clone())
        .map_err(|e| ApiError::bad_request(format!("invalid config: {e}")))?;

    access::require_cluster_write(pool, p, input.id).await?;

    sqlx::query("INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)")
        .bind(input.id)
        .bind(&json)
        .execute(pool)
        .await
        .map_err(internal)?;
    crate::api::push::notify_global(input.id, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

/// was: get_config_schema() in web/components/config_editor_panel.rs
#[api_mcp_dioxus_server(server = "get_config_schema")]
pub async fn cluster_config_schema(
    _pool: &sqlx::PgPool,
    _p: &Principal,
    _input: ConfigSchemaInput,
) -> Result<serde_json::Value, ApiError> {
    let schema = schemars::schema_for!(mac_mgmt_common::ClusterConfig);
    serde_json::to_value(&schema).map_err(internal)
}

/// was: get_config_history() in web/components/config_history.rs
#[api_mcp_dioxus_server(server = "get_config_history")]
pub async fn cluster_config_history(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ConfigHistoryInput,
) -> Result<Vec<ConfigVersion>, ApiError> {
    access::require_cluster_read(pool, p, input.id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, created_at FROM cluster_configs WHERE cluster_id = $1 \
         ORDER BY created_at DESC LIMIT 50",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(internal)?;

    Ok(rows
        .into_iter()
        .map(|r| ConfigVersion {
            id: r.id,
            created_at: r.created_at,
        })
        .collect())
}

/// was: get_config_diff() in web/components/config_history.rs
#[api_mcp_dioxus_server(server = "get_config_diff")]
pub async fn cluster_config_diff(
    pool: &sqlx::PgPool,
    p: &Principal,
    input: ConfigDiffInput,
) -> Result<Vec<DiffLine>, ApiError> {
    use similar::{ChangeTag, TextDiff};

    // Access is checked against the left revision's cluster (both revisions
    // of a comparison belong to the same cluster in practice).
    if let Some(ids) = access::accessible_cluster_ids(pool, p).await? {
        let cluster_id: Uuid =
            sqlx::query_scalar("SELECT cluster_id FROM cluster_configs WHERE id = $1")
                .bind(input.left_id)
                .fetch_optional(pool)
                .await
                .map_err(internal)?
                .ok_or_else(|| ApiError::not_found("config revision not found"))?;
        if !ids.contains(&cluster_id) {
            return Err(ApiError::forbidden("access denied"));
        }
    }

    let left_json: serde_json::Value =
        sqlx::query_scalar("SELECT config_json FROM cluster_configs WHERE id = $1")
            .bind(input.left_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .ok_or_else(|| ApiError::not_found("config revision not found"))?;
    let left_text = serde_json::to_string_pretty(&left_json).unwrap_or_default();

    let right_json: serde_json::Value =
        sqlx::query_scalar("SELECT config_json FROM cluster_configs WHERE id = $1")
            .bind(input.right_id)
            .fetch_optional(pool)
            .await
            .map_err(internal)?
            .ok_or_else(|| ApiError::not_found("config revision not found"))?;
    let right_text = serde_json::to_string_pretty(&right_json).unwrap_or_default();

    let diff = TextDiff::from_lines(&left_text, &right_text);
    let lines: Vec<DiffLine> = diff
        .iter_all_changes()
        .map(|change| {
            let tag = match change.tag() {
                ChangeTag::Equal => "equal",
                ChangeTag::Insert => "insert",
                ChangeTag::Delete => "delete",
            };
            DiffLine {
                tag: tag.to_string(),
                content: change.value().to_string(),
            }
        })
        .collect();

    Ok(lines)
}
