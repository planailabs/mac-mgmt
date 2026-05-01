use std::collections::HashMap;

use mac_mgmt_common::{
    FederationCatalog, FederationBundle, FederationBundleSkill, FederationEvent,
    FederationMcpBundle, FederationMcpBundleServer, FederationMcpServer,
    FederationSkillChannel, McpServerEntry, ResolveSkillsRequest, ResolveMcpServersRequest,
};
use rocket::http::Status;
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::{Shutdown, State, get, post};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::auth::{FederationAuth, FederationOrPublic};
use super::push::FederationPushChannel;

// ── Catalog endpoint ────────────────────────────────────────────────

#[get("/federation/catalog")]
pub async fn federation_catalog(
    auth: FederationOrPublic,
    pool: &State<PgPool>,
) -> Result<Json<FederationCatalog>, Status> {
    let include_hidden = auth.is_authenticated;

    let skill_channels = fetch_skill_channels(pool.inner(), include_hidden).await?;
    let bundles = fetch_bundles(pool.inner(), include_hidden).await?;
    let mcp_servers = fetch_mcp_servers(pool.inner(), include_hidden).await?;
    let mcp_bundles = fetch_mcp_bundles(pool.inner(), include_hidden).await?;

    Ok(Json(FederationCatalog {
        skill_channels,
        bundles,
        mcp_servers,
        mcp_bundles,
    }))
}

#[derive(sqlx::FromRow)]
struct SkillChannelDbRow {
    id: Uuid,
    skill_slug: String,
    skill_name: String,
    skill_description: String,
    channel: String,
    hidden: bool,
    nix_packages: Vec<String>,
}

async fn fetch_skill_channels(
    pool: &PgPool,
    include_hidden: bool,
) -> Result<Vec<FederationSkillChannel>, Status> {
    let filter = if include_hidden { "TRUE" } else { "NOT s.hide_from_public_catalog" };
    let query = format!(
        "SELECT sc.id, s.slug AS skill_slug, s.name AS skill_name, \
         s.description AS skill_description, sc.channel, \
         s.hide_from_public_catalog AS hidden, sc.nix_packages \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE {filter} \
         ORDER BY s.slug, sc.channel"
    );
    let rows: Vec<SkillChannelDbRow> = sqlx::query_as(&query)
        .fetch_all(pool)
        .await
        .map_err(|_| Status::InternalServerError)?;

    // Fetch MCP dependency slugs per skill channel.
    let channel_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut mcp_deps: HashMap<Uuid, Vec<String>> = HashMap::new();
    if !channel_ids.is_empty() {
        #[derive(sqlx::FromRow)]
        struct McpDepRow {
            skill_channel_id: Uuid,
            slug: String,
        }
        let dep_rows: Vec<McpDepRow> = sqlx::query_as(
            "SELECT smd.skill_channel_id, ms.slug \
             FROM skill_mcp_dependencies smd \
             JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
             WHERE smd.skill_channel_id = ANY($1) \
             ORDER BY ms.slug",
        )
        .bind(&channel_ids)
        .fetch_all(pool)
        .await
        .map_err(|_| Status::InternalServerError)?;
        for dep in dep_rows {
            mcp_deps.entry(dep.skill_channel_id).or_default().push(dep.slug);
        }
    }

    Ok(rows
        .into_iter()
        .map(|r| {
            let mcp_server_slugs = mcp_deps.remove(&r.id).unwrap_or_default();
            FederationSkillChannel {
                id: r.id,
                skill_slug: r.skill_slug,
                skill_name: r.skill_name,
                skill_description: r.skill_description,
                channel: r.channel,
                hidden: r.hidden,
                nix_packages: r.nix_packages,
                mcp_server_slugs,
            }
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct BundleDbRow {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    hidden: bool,
}

#[derive(sqlx::FromRow)]
struct BundleItemDbRow {
    bundle_id: Uuid,
    skill_channel_id: Uuid,
    skill_slug: String,
    channel: String,
}

async fn fetch_bundles(
    pool: &PgPool,
    include_hidden: bool,
) -> Result<Vec<FederationBundle>, Status> {
    let filter = if include_hidden { "TRUE" } else { "NOT b.hide_from_public_catalog" };
    let query = format!(
        "SELECT b.id, b.slug, b.name, b.description, \
         b.hide_from_public_catalog AS hidden \
         FROM bundles b WHERE {filter} ORDER BY b.slug"
    );
    let bundles: Vec<BundleDbRow> = sqlx::query_as(&query)
        .fetch_all(pool)
        .await
        .map_err(|_| Status::InternalServerError)?;

    let items: Vec<BundleItemDbRow> = sqlx::query_as(
        "SELECT bi.bundle_id, bi.skill_channel_id, s.slug AS skill_slug, sc.channel \
         FROM bundle_items bi \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let mut items_by_bundle: HashMap<Uuid, Vec<FederationBundleSkill>> = HashMap::new();
    for item in items {
        items_by_bundle
            .entry(item.bundle_id)
            .or_default()
            .push(FederationBundleSkill {
                skill_channel_id: item.skill_channel_id,
                skill_slug: item.skill_slug,
                channel: item.channel,
            });
    }

    Ok(bundles
        .into_iter()
        .map(|b| FederationBundle {
            id: b.id,
            slug: b.slug,
            name: b.name,
            description: b.description,
            hidden: b.hidden,
            skills: items_by_bundle.remove(&b.id).unwrap_or_default(),
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct McpServerDbRow {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    hidden: bool,
    config_json: serde_json::Value,
    nix_packages: Vec<String>,
}

async fn fetch_mcp_servers(
    pool: &PgPool,
    include_hidden: bool,
) -> Result<Vec<FederationMcpServer>, Status> {
    let filter = if include_hidden {
        "TRUE"
    } else {
        "NOT ms.hide_from_public_catalog"
    };
    let query = format!(
        "SELECT ms.id, ms.slug, ms.name, ms.description, \
         ms.hide_from_public_catalog AS hidden, ms.config_json, ms.nix_packages \
         FROM mcp_servers ms WHERE {filter} ORDER BY ms.slug"
    );
    let rows: Vec<McpServerDbRow> = sqlx::query_as(&query)
        .fetch_all(pool)
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok(rows
        .into_iter()
        .map(|r| FederationMcpServer {
            id: r.id,
            slug: r.slug,
            name: r.name,
            description: r.description,
            hidden: r.hidden,
            config: r.config_json,
            nix_packages: r.nix_packages,
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct McpBundleItemDbRow {
    bundle_id: Uuid,
    mcp_server_id: Uuid,
    slug: String,
}

async fn fetch_mcp_bundles(
    pool: &PgPool,
    include_hidden: bool,
) -> Result<Vec<FederationMcpBundle>, Status> {
    let filter = if include_hidden {
        "TRUE"
    } else {
        "NOT msb.hide_from_public_catalog"
    };
    let query = format!(
        "SELECT msb.id, msb.slug, msb.name, msb.description, \
         msb.hide_from_public_catalog AS hidden \
         FROM mcp_server_bundles msb WHERE {filter} ORDER BY msb.slug"
    );
    let bundles: Vec<BundleDbRow> = sqlx::query_as(&query)
        .fetch_all(pool)
        .await
        .map_err(|_| Status::InternalServerError)?;

    let items: Vec<McpBundleItemDbRow> = sqlx::query_as(
        "SELECT msbi.bundle_id, msbi.mcp_server_id, ms.slug \
         FROM mcp_server_bundle_items msbi \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| Status::InternalServerError)?;

    let mut items_by_bundle: HashMap<Uuid, Vec<FederationMcpBundleServer>> = HashMap::new();
    for item in items {
        items_by_bundle
            .entry(item.bundle_id)
            .or_default()
            .push(FederationMcpBundleServer {
                mcp_server_id: item.mcp_server_id,
                slug: item.slug,
            });
    }

    Ok(bundles
        .into_iter()
        .map(|b| FederationMcpBundle {
            id: b.id,
            slug: b.slug,
            name: b.name,
            description: b.description,
            hidden: b.hidden,
            servers: items_by_bundle.remove(&b.id).unwrap_or_default(),
        })
        .collect())
}

// ── Resolve endpoints ───────────────────────────────────────────────

#[post("/federation/resolve-skills", data = "<body>")]
pub async fn federation_resolve_skills(
    _auth: FederationAuth,
    body: Json<ResolveSkillsRequest>,
) -> Result<Json<HashMap<String, String>>, Status> {
    let cfg = crate::config::config();
    let xzar = cfg.xzar.as_ref().ok_or(Status::ServiceUnavailable)?;

    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|_| Status::InternalServerError)?;

    let skills: Vec<(String, String)> = body
        .skills
        .iter()
        .map(|s| (s.slug.clone(), s.channel.clone()))
        .collect();

    let result = crate::xzar::resolve_store_paths(&pins, &skills, &body.arch);
    Ok(Json(result))
}

#[post("/federation/resolve-mcp-servers", data = "<body>")]
pub async fn federation_resolve_mcp_servers(
    _auth: FederationAuth,
    pool: &State<PgPool>,
    body: Json<ResolveMcpServersRequest>,
) -> Result<Json<HashMap<String, McpServerEntry>>, Status> {
    #[derive(sqlx::FromRow)]
    struct Row {
        slug: String,
        config_json: serde_json::Value,
        nix_packages: Vec<String>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT slug, config_json, nix_packages FROM mcp_servers WHERE slug = ANY($1)",
    )
    .bind(&body.slugs)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let mut result = HashMap::new();
    for row in rows {
        result.insert(
            row.slug,
            McpServerEntry {
                config: row.config_json,
                nix_packages: row.nix_packages,
            },
        );
    }
    Ok(Json(result))
}

// ── Federation SSE endpoint ─────────────────────────────────────────

#[get("/federation/events?<token>")]
pub async fn federation_events(
    token: &str,
    pool: &State<PgPool>,
    federation_push: &State<FederationPushChannel>,
    mut shutdown: Shutdown,
) -> Option<EventStream![]> {
    // Authenticate the token
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let result = sqlx::query_as::<_, (String,)>(
        "SELECT kind FROM tokens WHERE token_hash = $1 AND NOT revoked \
         AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(&hash)
    .fetch_optional(pool.inner())
    .await
    .ok()??;

    if result.0 != "federation" {
        return None;
    }

    let rx = federation_push.subscribe();

    Some(EventStream! {
        let mut rx = rx;
        let mut keepalive = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Ok(event) => {
                            let json = serde_json::to_string(&event).unwrap_or_default();
                            yield Event::data(json);
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Federation SSE client lagged, skipped {n} messages");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = keepalive.tick() => {
                    let json = serde_json::to_string(&FederationEvent::Ping).unwrap_or_default();
                    yield Event::data(json);
                }
                _ = &mut shutdown => break,
            }
        }
    })
}
