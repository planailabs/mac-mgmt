//! Bearer-token `Authenticator` backed by the `tokens` table.
//!
//! Resolves a raw token to a framework `Principal`:
//! - `kind = 'admin'` → global admin (all orgs, read + write).
//! - `kind = 'api'` with an `organization_id` → scoped to that org; role comes
//!   from `scopes.role` (`"read"` | `"write"` | `"admin"`, default `"write"`).
//!   `"admin"` additionally grants org-admin via `scopes.org_admin`.
//! - `kind = 'api'` with `NULL` org → read-only across all orgs.
//! - `sync` / `setting` / `federation` / `proxy` → forbidden: those are
//!   daemon/relay-facing credentials for the legacy `/api` surface; letting
//!   them drive the management API would widen their blast radius.
//!
//! [`BearerAuthenticator`] puts this first and, when `[zitadel_bearer]` is
//! configured, falls back to Zitadel token introspection: a signed-in user's
//! Zitadel access token acts as that mac-mgmt user ([`ZitadelUsers`]).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use plan_ai_api_mcp::zitadel::{
    ClientAuth, Identity, IdentityMapper, IntrospectionConfig, ZitadelAuthenticator,
};
use plan_ai_api_mcp::{ApiError, Authenticator, OrgSet, Principal};
use sqlx::PgPool;
use uuid::Uuid;

use crate::web::auth::PgUserResolver;
use crate::web::user::principal_from;

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
        self.lookup(bearer)
            .await?
            .ok_or_else(|| ApiError::unauthorized("invalid or expired token"))
    }
}

impl TokenAuthenticator {
    /// The principal of a live `tokens` row with this bearer; `None` when
    /// there is none (unknown, revoked or expired).
    pub async fn lookup(&self, bearer: &str) -> Result<Option<Principal>, ApiError> {
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
        .map_err(|e| ApiError::internal(e.to_string()))?;
        let Some((kind, org, scopes)) = row else {
            return Ok(None);
        };
        Self::principal(kind, org, scopes).map(Some)
    }

    fn principal(
        kind: String,
        org: Option<Uuid>,
        scopes: Option<serde_json::Value>,
    ) -> Result<Principal, ApiError> {
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
                    let org_admin: Vec<Uuid> = if role == "admin" { vec![oid] } else { vec![] };
                    Ok(Principal {
                        admin: false,
                        read_orgs: OrgSet::Only(read),
                        write_orgs: OrgSet::Only(write),
                        scopes: serde_json::json!({ "org_admin": org_admin }),
                        subject: format!("token:api:{oid}"),
                    })
                }
                None => Ok(Principal {
                    admin: false,
                    read_orgs: OrgSet::All,
                    write_orgs: OrgSet::Only(HashSet::new()),
                    scopes: serde_json::json!({ "org_admin": [] }),
                    subject: "token:api:global-ro".into(),
                }),
            },
            other => Err(ApiError::forbidden(format!(
                "token kind '{other}' is not permitted on this API"
            ))),
        }
    }
}

/// Whether `bearer` has the shape of a token mac-mgmt mints: 64 lowercase hex
/// chars, optionally behind a `<letters>_` prefix (`fed_…`). Zitadel's opaque
/// access tokens never look like this, so such a bearer that is not in the
/// `tokens` table is rejected without asking Zitadel.
pub fn looks_like_mac_mgmt_token(bearer: &str) -> bool {
    let body = match bearer.split_once('_') {
        Some((prefix, rest)) if !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_lowercase()) => rest,
        _ => bearer,
    };
    body.len() == 64 && body.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The management API's authenticator: mac-mgmt's own tokens first (a cheap
/// hash lookup), then — when configured — Zitadel access tokens.
pub struct BearerAuthenticator {
    tokens: TokenAuthenticator,
    zitadel: Option<(ZitadelAuthenticator, Arc<dyn IdentityMapper>)>,
}

impl BearerAuthenticator {
    pub fn new(tokens: TokenAuthenticator) -> Self {
        Self { tokens, zitadel: None }
    }

    /// Also accept Zitadel access tokens: introspected per `cfg`, mapped to a
    /// principal by `mapper`. `http` defaults to a client with a 10s timeout.
    pub fn with_zitadel(
        mut self,
        cfg: IntrospectionConfig,
        mapper: Arc<dyn IdentityMapper>,
        http: Option<reqwest::Client>,
    ) -> Result<Self, plan_ai_api_mcp::zitadel::ZitadelError> {
        let auth = match http {
            Some(http) => ZitadelAuthenticator::with_http_client(http, cfg, mapper.clone()),
            None => ZitadelAuthenticator::new(cfg, mapper.clone())?,
        };
        self.zitadel = Some((auth, mapper));
        Ok(self)
    }
}

#[async_trait]
impl Authenticator for BearerAuthenticator {
    async fn authenticate(&self, bearer: &str) -> Result<Principal, ApiError> {
        if let Some(p) = self.tokens.lookup(bearer).await? {
            return Ok(p);
        }
        let Some((zitadel, mapper)) = &self.zitadel else {
            return Err(ApiError::unauthorized("invalid or expired token"));
        };
        if looks_like_mac_mgmt_token(bearer) {
            return Err(ApiError::unauthorized("invalid or expired token"));
        }
        let identity = zitadel.introspect(bearer).await.map_err(|e| match e {
            // Zitadel unreachable, or rejecting our introspection credentials
            // (logged by the introspection itself). To the caller this is a
            // bearer nobody could verify: 401, not a 500 for every bad token.
            ApiError::Internal(why) => {
                tracing::warn!(reason = %why, "zitadel bearer could not be verified");
                ApiError::unauthorized(
                    "invalid or expired token (not a mac-mgmt token, and Zitadel could not verify it)",
                )
            }
            other => other,
        })?;
        mapper.principal(&identity).await
    }
}

/// `[zitadel_bearer]` as the introspection config.
pub fn introspection_config(cfg: &crate::config::ZitadelBearerConfig) -> IntrospectionConfig {
    IntrospectionConfig {
        issuer: cfg.issuer.clone(),
        introspection_endpoint: None,
        client: ClientAuth::Basic {
            client_id: cfg.client_id.clone(),
            client_secret: cfg.client_secret.clone(),
        },
        project_id: cfg.project_id.clone(),
        cache_secs: 60,
    }
}

/// Maps a Zitadel identity to the mac-mgmt user with its email, through the
/// web session's user resolver ([`PgUserResolver::find_by_email`]) and the
/// session's principal ([`principal_from`]): the bearer acts with exactly
/// that user's permissions. Users are never created from a bearer.
pub struct ZitadelUsers {
    resolver: Arc<PgUserResolver>,
}

impl ZitadelUsers {
    pub fn new(resolver: Arc<PgUserResolver>) -> Self {
        Self { resolver }
    }
}

#[async_trait]
impl IdentityMapper for ZitadelUsers {
    async fn principal(&self, identity: &Identity) -> Result<Principal, ApiError> {
        let Some(email) = identity.email.as_deref().filter(|e| !e.is_empty()) else {
            return Err(ApiError::forbidden(
                "the Zitadel token carries no email; request it with the `email` scope",
            ));
        };
        if identity.email_verified == Some(false) {
            return Err(ApiError::forbidden(format!(
                "the Zitadel token's email {email} is not verified"
            )));
        }
        let user = self
            .resolver
            .find_by_email(email)
            .await
            .map_err(|e| ApiError::internal(format!("loading user {email}: {e}")))?
            .ok_or_else(|| {
                ApiError::forbidden(format!(
                    "no mac-mgmt user with email {email}; sign in to the mac-mgmt web UI once (or be invited) first"
                ))
            })?;
        let mut p = principal_from(&user);
        p.subject = format!("user:{}", user.email);
        p.scopes["zitadel_user_id"] = identity.user_id.clone().into();
        if let Some(actor) = &identity.actor {
            p.subject = format!("{} via {}", p.subject, actor.sub);
            p.scopes["actor"] = actor.sub.clone().into();
        }
        tracing::info!(
            target: "mac_mgmt::audit",
            user = %user.email,
            user_id = %user.id,
            zitadel_user_id = %identity.user_id,
            actor = identity.actor.as_ref().map(|a| a.sub.as_str()).unwrap_or(""),
            client_id = identity.client_id.as_deref().unwrap_or(""),
            "management API: zitadel bearer acts as user"
        );
        Ok(p)
    }
}

/// Bearer chain against a throwaway PostgreSQL (initdb/pg_ctl from PATH; the
/// test is skipped without them) with the real migrations, and a mocked
/// Zitadel introspection endpoint.
#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use dioxus::fullstack::axum::{self, Form, Json, Router, extract::State, routing::post};
    use serde_json::{Value, json};
    use std::process::Command;
    use std::sync::Mutex;

    const PROJECT: &str = "397603440676569224";

    struct TempPg {
        dir: std::path::PathBuf,
    }

    impl TempPg {
        fn start() -> Option<(Self, String)> {
            let dir = std::env::temp_dir().join(format!("mac-mgmt-auth-test-{}", Uuid::new_v4()));
            let data = dir.join("data");
            let init = Command::new("initdb")
                .args(["-A", "trust", "-U", "postgres", "-D"])
                .arg(&data)
                .output();
            match init {
                Ok(o) if o.status.success() => {}
                Ok(o) => panic!("initdb failed: {}", String::from_utf8_lossy(&o.stderr)),
                Err(e) => {
                    eprintln!("skipping: initdb not available ({e})");
                    return None;
                }
            }
            let opts = format!("-k {} -c listen_addresses='' -c fsync=off", dir.display());
            let ok = Command::new("pg_ctl")
                .args(["-w", "-D"])
                .arg(&data)
                .args(["-o", &opts, "-l"])
                .arg(dir.join("log"))
                .arg("start")
                .status()
                .expect("pg_ctl")
                .success();
            assert!(ok, "pg_ctl start failed; see {}", dir.join("log").display());
            let url = format!("postgres://postgres@localhost/postgres?host={}", dir.display());
            Some((Self { dir }, url))
        }
    }

    impl Drop for TempPg {
        fn drop(&mut self) {
            let _ = Command::new("pg_ctl")
                .args(["-w", "-m", "immediate", "-D"])
                .arg(self.dir.join("data"))
                .arg("stop")
                .output();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[derive(Clone, Default)]
    struct Seen(Arc<Mutex<Vec<String>>>);

    async fn introspect(State(seen): State<Seen>, Form(form): Form<Vec<(String, String)>>) -> Json<Value> {
        let token = form.iter().find(|(k, _)| k == "token").map(|(_, v)| v.clone()).unwrap_or_default();
        seen.0.lock().unwrap().push(token.clone());
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 300;
        let user = |email: &str| {
            json!({ "active": true, "sub": "z-1", "aud": [PROJECT, "app@mgmt"], "exp": exp,
                    "email": email, "email_verified": true, "client_id": "chat@plan",
                    "act": { "sub": "connector-sa" } })
        };
        Json(match token.as_str() {
            "zitadel-alice" => user("alice@example.com"),
            "zitadel-root" => user("root@example.com"),
            "zitadel-stranger" => user("stranger@example.com"),
            "zitadel-other-project" => json!({ "active": true, "sub": "z-1", "aud": ["999"], "exp": exp,
                                                "email": "alice@example.com" }),
            _ => json!({ "active": false }),
        })
    }

    async fn mock_zitadel() -> (String, Seen) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let seen = Seen::default();
        let app = Router::new()
            .route("/oauth/v2/introspect", post(introspect))
            .with_state(seen.clone());
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        (url, seen)
    }

    fn hash(token: &str) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(token.as_bytes()))
    }

    #[test]
    fn mac_mgmt_token_shape() {
        let raw = "0123456789abcdef".repeat(4);
        assert!(looks_like_mac_mgmt_token(&raw));
        assert!(looks_like_mac_mgmt_token(&format!("fed_{raw}")));
        assert!(!looks_like_mac_mgmt_token(&raw[1..]));
        assert!(!looks_like_mac_mgmt_token("zitadel-alice"));
        assert!(!looks_like_mac_mgmt_token(&raw.to_uppercase()));
    }

    #[tokio::test]
    async fn zitadel_bearers_act_as_the_mac_mgmt_user() {
        let Some((_pg, url)) = TempPg::start() else { return };
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        plan_ai_chat::store::migrations::run_migrations(&pool).await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let org_rw: Uuid = sqlx::query_scalar("INSERT INTO organizations (name) VALUES ('rw') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
        let org_ro: Uuid = sqlx::query_scalar("INSERT INTO organizations (name) VALUES ('ro') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
        let org_adm: Uuid = sqlx::query_scalar("INSERT INTO organizations (name) VALUES ('adm') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
        let alice: Uuid = sqlx::query_scalar(
            "INSERT INTO users (email, name) VALUES ('alice@example.com', 'Alice') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        for (org, role) in [(org_rw, "write"), (org_ro, "read"), (org_adm, "admin")] {
            sqlx::query("INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1, $2, $3)")
                .bind(org)
                .bind(alice)
                .bind(role)
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO users (email, name) VALUES ('root@example.com', 'Root')")
            .execute(&pool)
            .await
            .unwrap();
        let api_token = "ab".repeat(32);
        sqlx::query("INSERT INTO tokens (token_hash, kind, organization_id, scopes) VALUES ($1, 'api', $2, $3)")
            .bind(hash(&api_token))
            .bind(org_rw)
            .bind(json!({ "role": "read" }))
            .execute(&pool)
            .await
            .unwrap();

        let (issuer, seen) = mock_zitadel().await;
        let cfg = crate::config::ZitadelBearerConfig {
            issuer,
            project_id: PROJECT.into(),
            client_id: "397603441129619592".into(),
            client_secret: "secret".into(),
        };
        let resolver = PgUserResolver::new(pool.clone(), vec!["root@example.com".into()]);
        let auth = BearerAuthenticator::new(TokenAuthenticator::new(pool.clone()))
            .with_zitadel(introspection_config(&cfg), Arc::new(ZitadelUsers::new(resolver)), None)
            .unwrap();

        // Zitadel bearer → alice's principal, as her web session builds it.
        let p = auth.authenticate("zitadel-alice").await.unwrap();
        assert_eq!(p.subject, "user:alice@example.com via connector-sa");
        assert!(!p.admin);
        for org in [org_rw, org_ro, org_adm] {
            assert!(p.read_orgs.contains(&org), "read {org}");
        }
        assert!(p.write_orgs.contains(&org_rw));
        assert!(p.write_orgs.contains(&org_adm));
        assert!(!p.write_orgs.contains(&org_ro));
        assert_eq!(p.scopes["org_admin"], json!([org_adm]));
        assert_eq!(p.scopes["actor"], "connector-sa");
        assert_eq!(p.scopes["zitadel_user_id"], "z-1");

        // An admin_emails address is admin, as at web sign-in.
        let p = auth.authenticate("zitadel-root").await.unwrap();
        assert!(p.admin);

        // Unknown user: 403, and no user is created.
        let e = auth.authenticate("zitadel-stranger").await.unwrap_err();
        assert!(matches!(e, ApiError::Forbidden(ref m) if m.contains("stranger@example.com")), "{e}");
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email = 'stranger@example.com'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0);

        // Wrong audience / inactive: 401.
        let e = auth.authenticate("zitadel-other-project").await.unwrap_err();
        assert!(matches!(e, ApiError::Unauthorized(ref m) if m.contains(PROJECT)), "{e}");
        let e = auth.authenticate("garbage").await.unwrap_err();
        assert!(matches!(e, ApiError::Unauthorized(_)), "{e}");

        // mac-mgmt tokens still work, and never reach Zitadel; nor does an
        // unknown token of mac-mgmt's shape.
        let before = seen.0.lock().unwrap().len();
        let p = auth.authenticate(&api_token).await.unwrap();
        assert_eq!(p.subject, format!("token:api:{org_rw}"));
        assert!(p.read_orgs.contains(&org_rw) && !p.write_orgs.contains(&org_rw));
        let e = auth.authenticate(&"cd".repeat(32)).await.unwrap_err();
        assert!(matches!(e, ApiError::Unauthorized(_)), "{e}");
        assert_eq!(seen.0.lock().unwrap().len(), before);

        // Zitadel unreachable: a bad bearer is still a 401, not a 500.
        let down = crate::config::ZitadelBearerConfig { issuer: "http://127.0.0.1:9".into(), ..cfg };
        let resolver = PgUserResolver::new(pool.clone(), vec![]);
        let auth = BearerAuthenticator::new(TokenAuthenticator::new(pool.clone()))
            .with_zitadel(introspection_config(&down), Arc::new(ZitadelUsers::new(resolver)), None)
            .unwrap();
        let e = auth.authenticate("zitadel-alice").await.unwrap_err();
        assert!(matches!(e, ApiError::Unauthorized(_)), "{e}");
    }
}
