//! Cluster/org permission helpers over a framework `Principal`.
//!
//! Mirrors the semantics of `web::user::WebUserExt` (the cookie-session
//! equivalents): cluster access derives from org membership via the
//! `organization_clusters` join; org-admin rides in `Principal.scopes`.

use plan_ai_api_mcp::{ApiError, Principal};
use sqlx::PgPool;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::error::{DatabaseError, ErrorKind};

    /// Minimal stand-in for sqlx's database error, so the SQLSTATE mapping is
    /// tested without a database.
    #[derive(Debug)]
    struct DbErr(String);

    impl std::fmt::Display for DbErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for DbErr {}

    impl DatabaseError for DbErr {
        fn message(&self) -> &str {
            &self.0
        }
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
        fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
            Some(std::borrow::Cow::Borrowed(&self.0))
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
        fn constraint(&self) -> Option<&str> {
            None
        }
    }

    fn db_err(sqlstate: &str) -> sqlx::Error {
        sqlx::Error::Database(Box::new(DbErr(sqlstate.to_string())))
    }

    #[test]
    fn db_write_maps_constraint_violations() {
        let status = |e| db_write(e, "cluster").status().as_u16();
        // 23503 foreign key: a referenced id that does not exist is a 404, not a 500.
        assert_eq!(status(db_err("23503")), 404);
        // 23505 unique violation.
        assert_eq!(status(db_err("23505")), 409);
        // Anything else stays an internal error.
        assert_eq!(status(db_err("08006")), 500);
        // No row returned by an `UPDATE … RETURNING`: the id is simply unknown.
        assert_eq!(status(sqlx::Error::RowNotFound), 404);
    }

    #[test]
    fn write_scope_checks_existence_for_admins() {
        let cid = Uuid::new_v4();
        // Admin: no org limit, so the cluster's existence still has to be
        // confirmed — that is what turns a bogus id into a 404 and not a 500
        // (t4t finding #4, POST /clusters/{id}/cloud_init).
        assert!(matches!(
            write_scope(&None, cid),
            Ok(WriteScope::Everywhere)
        ));
        // A member of an org owning the cluster writes to it.
        assert!(matches!(
            write_scope(&Some(vec![cid]), cid),
            Ok(WriteScope::ThisCluster)
        ));
        // A member of another org is refused, not redirected to a 404: the
        // caller must not learn whether a cluster it cannot see exists.
        let err = write_scope(&Some(vec![Uuid::new_v4()]), cid).unwrap_err();
        assert_eq!(err.status().as_u16(), 403, "expected 403, got {err:?}");
    }

    #[test]
    fn missing_import_source_is_not_found() {
        // import_jobs.source_id FK on POST …/trigger_sync (t4t finding #12).
        let err = db_write(db_err("23503"), "import source");
        assert_eq!(err.status().as_u16(), 404, "expected 404, got {err:?}");
        assert!(
            matches!(err, ApiError::NotFound(m) if m == "import source not found"),
            "message must name the missing source"
        );
    }
}

fn db(e: sqlx::Error) -> ApiError {
    ApiError::internal(e.to_string())
}

/// A write that referenced a row that does not exist.
///
/// Postgres reports a violated foreign key as a generic database error, which
/// used to surface as a 500 ("internal: error returned from database: …"), so
/// any endpoint that binds a caller-supplied id leaked a server error for a
/// plain 404/409 case. Use this on the insert/update paths that bind ids from
/// the request body; don't use it for queries where the only FK risk is an
/// internal one.
pub fn db_write(e: sqlx::Error, what: &str) -> ApiError {
    match &e {
        sqlx::Error::Database(dbe) if dbe.code().as_deref() == Some("23503") => {
            ApiError::not_found(format!("{what} not found"))
        }
        sqlx::Error::Database(dbe) if dbe.code().as_deref() == Some("23505") => {
            ApiError::conflict(format!("{what} already exists"))
        }
        sqlx::Error::RowNotFound => ApiError::not_found(format!("{what} not found")),
        _ => db(e),
    }
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
///
/// For a principal that is allowed everywhere (admin), the id comes straight
/// from the request, so confirm the cluster exists: otherwise the write this
/// guards fails on its foreign key and surfaces as a 500 (t4t finding #4,
/// `POST /clusters/{id}/cloud_init`). Members are already limited to clusters
/// that exist, so a cluster outside their orgs keeps its 403.
pub async fn require_cluster_write(
    pool: &PgPool,
    p: &Principal,
    cluster_id: Uuid,
) -> Result<(), ApiError> {
    let ids = writable_cluster_ids(pool, p).await?;
    match write_scope(&ids, cluster_id)? {
        WriteScope::Everywhere => require_existing_cluster(pool, cluster_id).await,
        WriteScope::ThisCluster => Ok(()),
    }
}

/// The part of the write check that needs no database: `Everywhere` (admin —
/// the cluster's existence still has to be confirmed), `ThisCluster` when
/// membership allows it, `Err` when it does not.
fn write_scope(ids: &Option<Vec<Uuid>>, cluster_id: Uuid) -> Result<WriteScope, ApiError> {
    match ids {
        None => Ok(WriteScope::Everywhere),
        Some(ids) if ids.contains(&cluster_id) => Ok(WriteScope::ThisCluster),
        Some(_) => Err(ApiError::forbidden("write access denied")),
    }
}

#[derive(Debug)]
enum WriteScope {
    Everywhere,
    ThisCluster,
}

async fn require_existing_cluster(pool: &PgPool, cluster_id: Uuid) -> Result<(), ApiError> {
    let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM clusters WHERE id = $1")
        .bind(cluster_id)
        .fetch_optional(pool)
        .await
        .map_err(db)?;
    found
        .map(|_| ())
        .ok_or_else(|| ApiError::not_found("cluster not found"))
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
