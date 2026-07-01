//! Bearer-token `Authenticator` backed by the `tokens` table.
//!
//! Resolves a raw token to a framework `Principal`:
//! - `kind = 'admin'` → global admin (all orgs, read + write).
//! - `kind = 'api'` with an `organization_id` → scoped to that org; role comes
//!   from `scopes.role` (`"read"` | `"write"`, default `"write"`).
//! - `kind = 'api'` with `NULL` org → read-only across all orgs.
//! - any other kind → forbidden for this API surface.

use std::collections::HashSet;

use async_trait::async_trait;
use plan_ai_api_mcp::{ApiError, Authenticator, OrgSet, Principal};
use sqlx::PgPool;
use uuid::Uuid;

pub struct TokenAuthenticator {
    pool: PgPool,
}

impl TokenAuthenticator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Authenticator for TokenAuthenticator {
    async fn authenticate(&self, bearer: &str) -> Result<Principal, ApiError> {
        use sha2::{Digest, Sha256};
        let hash = hex::encode(Sha256::digest(bearer.as_bytes()));

        let row = sqlx::query_as::<_, (String, Option<Uuid>, Option<serde_json::Value>)>(
            "SELECT kind, organization_id, scopes FROM tokens \
             WHERE token_hash = $1 AND NOT revoked \
               AND (expires_at IS NULL OR expires_at > now())",
        )
        .bind(&hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::unauthorized("invalid or expired token"))?;

        let (kind, org, scopes) = row;
        match kind.as_str() {
            "admin" => Ok(Principal::admin("token:admin")),
            "api" => match org {
                Some(oid) => {
                    let role = scopes
                        .as_ref()
                        .and_then(|s| s.get("role"))
                        .and_then(|r| r.as_str())
                        .unwrap_or("write");
                    let read = HashSet::from([oid]);
                    let write = if role == "read" {
                        HashSet::new()
                    } else {
                        HashSet::from([oid])
                    };
                    Ok(Principal {
                        admin: false,
                        read_orgs: OrgSet::Only(read),
                        write_orgs: OrgSet::Only(write),
                        scopes: scopes.unwrap_or(serde_json::Value::Null),
                        subject: format!("token:api:{oid}"),
                    })
                }
                None => Ok(Principal {
                    admin: false,
                    read_orgs: OrgSet::All,
                    write_orgs: OrgSet::Only(HashSet::new()),
                    scopes: serde_json::Value::Null,
                    subject: "token:api:global-ro".into(),
                }),
            },
            other => Err(ApiError::forbidden(format!(
                "token kind '{other}' is not permitted on this API"
            ))),
        }
    }
}
