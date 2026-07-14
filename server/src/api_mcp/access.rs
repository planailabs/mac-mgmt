//! Cluster/org permission helpers over a framework `Principal`.
//!
//! Mirrors the semantics of `web::user::WebUserExt` (the cookie-session
//! equivalents): cluster access derives from org membership via the
//! `organization_clusters` join; org-admin rides in `Principal.scopes`.

use plan_ai_api_mcp::{ApiError, Principal};
use sqlx::PgPool;
use uuid::Uuid;

fn db(e: sqlx::Error) -> ApiError {
    ApiError::internal(e.to_string())
}

/// Org IDs the principal is an org-admin of (from `scopes.org_admin`).
/// Global admins are org-admin everywhere and never consult this.
fn org_admin_ids(p: &Principal) -> Vec<Uuid> {
    p.scopes
        .get("org_admin")
        .and_then(|v| serde_json::from_value::<Vec<Uuid>>(v.clone()).ok())
        .unwrap_or_default()
}

/// Whether the principal is an admin of the given org.
pub fn is_org_admin(p: &Principal, org_id: &Uuid) -> bool {
    p.admin || org_admin_ids(p).contains(org_id)
}

/// Require org-admin (or global admin).
pub fn require_org_admin(p: &Principal, org_id: &Uuid) -> Result<(), ApiError> {
    if is_org_admin(p, org_id) {
        Ok(())
    } else {
        Err(ApiError::forbidden("organization admin access required"))
    }
}

/// Clusters readable by the principal. `None` = all (admin, or read-all token).
pub async fn accessible_cluster_ids(
    pool: &PgPool,
    p: &Principal,
) -> Result<Option<Vec<Uuid>>, ApiError> {
    let Some(org_ids) = p.read_filter() else {
        return Ok(None);
    };
    if org_ids.is_empty() {
        return Ok(Some(vec![]));
    }
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT DISTINCT oc.cluster_id \
         FROM organization_clusters oc \
         WHERE oc.organization_id = ANY($1)",
    )
    .bind(&org_ids)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    Ok(Some(ids))
}

/// Clusters writable by the principal. `None` = all (admin).
pub async fn writable_cluster_ids(
    pool: &PgPool,
    p: &Principal,
) -> Result<Option<Vec<Uuid>>, ApiError> {
    if p.admin {
        return Ok(None);
    }
    let org_ids: Vec<Uuid> = match &p.write_orgs {
        plan_ai_api_mcp::OrgSet::All => return Ok(None),
        plan_ai_api_mcp::OrgSet::Only(set) => set.iter().copied().collect(),
    };
    if org_ids.is_empty() {
        return Ok(Some(vec![]));
    }
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT DISTINCT oc.cluster_id \
         FROM organization_clusters oc \
         WHERE oc.organization_id = ANY($1)",
    )
    .bind(&org_ids)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    Ok(Some(ids))
}

/// Require read access to a cluster.
pub async fn require_cluster_read(
    pool: &PgPool,
    p: &Principal,
    cluster_id: Uuid,
) -> Result<(), ApiError> {
    if let Some(ids) = accessible_cluster_ids(pool, p).await? {
        if !ids.contains(&cluster_id) {
            return Err(ApiError::forbidden("access denied"));
        }
    }
    Ok(())
}

/// Require write access to a cluster.
pub async fn require_cluster_write(
    pool: &PgPool,
    p: &Principal,
    cluster_id: Uuid,
) -> Result<(), ApiError> {
    if let Some(ids) = writable_cluster_ids(pool, p).await? {
        if !ids.contains(&cluster_id) {
            return Err(ApiError::forbidden("write access denied"));
        }
    }
    Ok(())
}

/// Whether the principal is org-admin of any org owning the cluster.
pub async fn is_cluster_org_admin(
    pool: &PgPool,
    p: &Principal,
    cluster_id: Uuid,
) -> Result<bool, ApiError> {
    if p.admin {
        return Ok(true);
    }
    let admin_orgs = org_admin_ids(p);
    if admin_orgs.is_empty() {
        return Ok(false);
    }
    let owning: Vec<Uuid> = sqlx::query_scalar(
        "SELECT organization_id FROM organization_clusters WHERE cluster_id = $1",
    )
    .bind(cluster_id)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    Ok(owning.iter().any(|o| admin_orgs.contains(o)))
}
