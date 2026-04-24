use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub struct AuthenticatedToken {
    pub cluster_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub token_kind: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedToken {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pool = match req.rocket().state::<PgPool>() {
            Some(p) => p,
            None => return Outcome::Error((Status::InternalServerError, "no database pool")),
        };

        let header = match req.headers().get_one("Authorization") {
            Some(h) => h,
            None => return Outcome::Error((Status::Unauthorized, "missing Authorization header")),
        };

        let token = match header.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return Outcome::Error((Status::Unauthorized, "invalid Authorization format")),
        };

        let hash = hex::encode(Sha256::digest(token.as_bytes()));

        let result = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>, String)>(
            "SELECT cluster_id, organization_id, kind FROM tokens \
             WHERE token_hash = $1 AND NOT revoked \
             AND (expires_at IS NULL OR expires_at > now())",
        )
        .bind(&hash)
        .fetch_optional(pool)
        .await;

        match result {
            Ok(Some((cluster_id, organization_id, kind))) => Outcome::Success(AuthenticatedToken {
                cluster_id,
                organization_id,
                token_kind: kind,
            }),
            Ok(None) => Outcome::Error((Status::Unauthorized, "invalid or revoked token")),
            Err(_) => Outcome::Error((Status::InternalServerError, "database error")),
        }
    }
}

/// Guard that only allows sync tokens.
pub struct SyncAuth {
    pub cluster_id: Uuid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SyncAuth {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match AuthenticatedToken::from_request(req).await {
            Outcome::Success(auth) if auth.token_kind == "sync" => match auth.cluster_id {
                Some(cid) => Outcome::Success(SyncAuth { cluster_id: cid }),
                None => Outcome::Error((Status::Forbidden, "sync token requires a cluster")),
            },
            Outcome::Success(_) => Outcome::Error((Status::Forbidden, "sync token required")),
            Outcome::Error(e) => Outcome::Error(e),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}

/// Guard that only allows setting tokens (including org-scoped setting tokens).
///
/// For single-cluster tokens, `cluster_id` comes from the token directly.
/// For org-scoped or admin tokens, `cluster_id` is resolved from the `X-Cluster-Id` header.
pub struct SettingAuth {
    pub cluster_id: Uuid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SettingAuth {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pool = match req.rocket().state::<PgPool>() {
            Some(p) => p,
            None => return Outcome::Error((Status::InternalServerError, "no database pool")),
        };

        match AuthenticatedToken::from_request(req).await {
            Outcome::Success(auth) if auth.token_kind == "setting" => {
                // Single-cluster setting token: use token's cluster_id directly
                if let Some(cid) = auth.cluster_id {
                    return Outcome::Success(SettingAuth { cluster_id: cid });
                }

                // Org-scoped setting token: resolve from X-Cluster-Id header
                if let Some(org_id) = auth.organization_id {
                    let header_cid = match parse_cluster_id_header(req) {
                        Ok(cid) => cid,
                        Err(e) => return Outcome::Error(e),
                    };

                    // Verify cluster belongs to the organization
                    let exists = sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS(SELECT 1 FROM organization_clusters WHERE organization_id = $1 AND cluster_id = $2)",
                    )
                    .bind(org_id)
                    .bind(header_cid)
                    .fetch_one(pool)
                    .await;

                    match exists {
                        Ok(true) => {
                            return Outcome::Success(SettingAuth {
                                cluster_id: header_cid,
                            });
                        }
                        Ok(false) => {
                            return Outcome::Error((
                                Status::Forbidden,
                                "cluster not in organization",
                            ));
                        }
                        Err(_) => {
                            return Outcome::Error((Status::InternalServerError, "database error"));
                        }
                    }
                }

                // Setting token with neither cluster_id nor organization_id — invalid
                Outcome::Error((
                    Status::Forbidden,
                    "setting token requires a cluster or organization",
                ))
            }
            Outcome::Success(auth) if auth.token_kind == "admin" => {
                // Admin token: resolve from X-Cluster-Id header
                let header_cid = match parse_cluster_id_header(req) {
                    Ok(cid) => cid,
                    Err(e) => return Outcome::Error(e),
                };

                // Verify cluster exists
                let exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM clusters WHERE id = $1)",
                )
                .bind(header_cid)
                .fetch_one(pool)
                .await;

                match exists {
                    Ok(true) => Outcome::Success(SettingAuth {
                        cluster_id: header_cid,
                    }),
                    Ok(false) => Outcome::Error((Status::NotFound, "cluster not found")),
                    Err(_) => Outcome::Error((Status::InternalServerError, "database error")),
                }
            }
            Outcome::Success(_) => Outcome::Error((Status::Forbidden, "setting token required")),
            Outcome::Error(e) => Outcome::Error(e),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}

/// Parse the `X-Cluster-Id` header as a UUID.
fn parse_cluster_id_header(req: &Request<'_>) -> Result<Uuid, (Status, &'static str)> {
    let header = req.headers().get_one("X-Cluster-Id").ok_or((
        Status::BadRequest,
        "X-Cluster-Id header required for org/admin tokens",
    ))?;
    header
        .parse::<Uuid>()
        .map_err(|_| (Status::BadRequest, "X-Cluster-Id must be a valid UUID"))
}

/// Scope resolved from a metrics-capable token.
pub enum MetricsScope {
    /// Single-cluster setting token.
    Cluster(Uuid),
    /// Org-scoped setting token — caller sees all clusters in the org.
    Organization(Uuid),
    /// Admin token — global view.
    All,
}

/// Guard that allows setting tokens (cluster or org-scoped) and admin tokens
/// for the `/metrics` endpoint. Unlike `SettingAuth`, no `X-Cluster-Id`
/// header is required — the scope is derived entirely from the token.
pub struct MetricsAuth {
    pub scope: MetricsScope,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for MetricsAuth {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match AuthenticatedToken::from_request(req).await {
            Outcome::Success(auth) if auth.token_kind == "setting" => {
                if let Some(cid) = auth.cluster_id {
                    Outcome::Success(MetricsAuth {
                        scope: MetricsScope::Cluster(cid),
                    })
                } else if let Some(org_id) = auth.organization_id {
                    Outcome::Success(MetricsAuth {
                        scope: MetricsScope::Organization(org_id),
                    })
                } else {
                    Outcome::Error((
                        Status::Forbidden,
                        "setting token requires a cluster or organization",
                    ))
                }
            }
            Outcome::Success(auth) if auth.token_kind == "admin" => Outcome::Success(MetricsAuth {
                scope: MetricsScope::All,
            }),
            Outcome::Success(_) => {
                Outcome::Error((Status::Forbidden, "setting or admin token required"))
            }
            Outcome::Error(e) => Outcome::Error(e),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}

/// Guard that only allows admin tokens.
pub struct AdminAuth;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminAuth {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match AuthenticatedToken::from_request(req).await {
            Outcome::Success(auth) if auth.token_kind == "admin" => Outcome::Success(AdminAuth),
            Outcome::Success(_) => Outcome::Error((Status::Forbidden, "admin token required")),
            Outcome::Error(e) => Outcome::Error(e),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}
