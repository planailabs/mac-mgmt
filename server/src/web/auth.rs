use dioxus::fullstack::axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_oidc_client::{
    auth::{AuthLayer, OAuthConfiguration, SESSION_KEY},
    auth_builder::OAuthConfigurationBuilder,
    auth_cache::AuthCache,
    cache::{TwoTierAuthCache, config::TwoTierCacheConfig},
    logout::handle_default_logout::DefaultLogoutHandler,
    sql_cache::{SqlAuthCache, SqlCacheConfig},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::config;
use super::user::{OrgMembership, WebUser};

/// Extract the email from a JWT ID token's payload (base64url-decoded, no verification needed
/// since the OIDC client already validated it).
fn email_from_id_token(id_token: &str) -> Option<String> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload = engine.decode(parts[1]).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    claims.get("email")?.as_str().map(String::from)
}

/// Extract the user's display name from the JWT ID token payload.
fn name_from_id_token(id_token: &str) -> Option<String> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload = engine.decode(parts[1]).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    claims.get("name")?.as_str().map(String::from)
}

/// Build the OIDC AuthLayer and session cache from config.
pub async fn build_auth_layer(
    db_url: &str,
) -> (AuthLayer, Arc<dyn AuthCache + Send + Sync>) {
    let cfg = config::config().oidc.as_ref()
        .expect("build_auth_layer called without OIDC config");

    let oauth_config = OAuthConfigurationBuilder::default()
        .with_issuer("https://accounts.google.com")
        .await
        .expect("failed to discover Google OIDC endpoints")
        .with_client_id(&cfg.client_id)
        .with_client_secret(&cfg.client_secret)
        .with_redirect_uri(&cfg.redirect_uri)
        .with_private_cookie_key(&cfg.cookie_secret)
        .with_scopes(vec!["openid", "email", "profile"])
        .with_post_logout_redirect_uri("/auth/login")
        // axum-oidc-client 0.3.0 has a units bug: handle_default calls
        // extend_auth_session(id, session_max_age) which treats the value as
        // seconds, while with_session_max_age is documented as minutes. We pass
        // 21600 so the server-side cache row lives for 6h sliding (21600 sec).
        // The cookie max_age becomes Duration::minutes(21600) ≈ 15 days, but
        // the cache row is the real gate.
        .with_session_max_age(21600)
        .build()
        .expect("failed to build OIDC configuration");

    let cache: Arc<dyn AuthCache + Send + Sync> = if let Some(redis_url) = &cfg.redis_url {
        // Redis L2 with Moka L1
        let redis_cache = axum_oidc_client::redis::AuthCache::new(redis_url, 28800);
        Arc::new(
            TwoTierAuthCache::new(
                Some(Arc::new(redis_cache)),
                TwoTierCacheConfig::default(),
            )
            .expect("failed to create two-tier cache with Redis"),
        )
    } else {
        // PostgreSQL L2 with Moka L1
        let sql_config = SqlCacheConfig {
            connection_string: db_url.to_string(),
            ..Default::default()
        };
        let sql_cache = SqlAuthCache::new(sql_config)
            .await
            .expect("failed to create PostgreSQL session cache");
        sql_cache
            .init_schema()
            .await
            .expect("failed to init OIDC cache schema");
        Arc::new(
            TwoTierAuthCache::new(
                Some(Arc::new(sql_cache)),
                TwoTierCacheConfig::default(),
            )
            .expect("failed to create two-tier cache with PostgreSQL"),
        )
    };

    let logout_handler = Arc::new(DefaultLogoutHandler);
    let auth_layer = AuthLayer::new(Arc::new(oauth_config), cache.clone(), logout_handler);

    (auth_layer, cache)
}

/// Upsert the user record and load org memberships.
async fn resolve_user(
    pool: &sqlx::PgPool,
    email: &str,
    name: Option<&str>,
) -> Result<WebUser, sqlx::Error> {
    let oidc = config::config().oidc.as_ref();
    let is_admin_email = oidc.is_some_and(|o| o.admin_emails.contains(&email.to_string()));

    // Upsert user: create on first login, update name on subsequent logins.
    // If the email is in admin_emails, ensure is_admin is set to true.
    let display_name = name.unwrap_or("");
    let user = if is_admin_email {
        sqlx::query_as::<_, (Uuid, String, String, bool)>(
            "INSERT INTO users (email, name, is_admin) VALUES ($1, $2, true) \
             ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name, is_admin = true \
             RETURNING id, email, name, is_admin",
        )
        .bind(email)
        .bind(display_name)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as::<_, (Uuid, String, String, bool)>(
            "INSERT INTO users (email, name) VALUES ($1, $2) \
             ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name \
             RETURNING id, email, name, is_admin",
        )
        .bind(email)
        .bind(display_name)
        .fetch_one(pool)
        .await?
    };

    // Load organization memberships with roles
    let org_memberships = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT organization_id, role FROM organization_members WHERE user_id = $1",
    )
    .bind(user.0)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(org_id, role)| OrgMembership { org_id, role })
    .collect();

    Ok(WebUser {
        id: user.0,
        email: user.1,
        name: user.2,
        is_admin: user.3,
        org_memberships,
        impersonating_from: None,
    })
}

/// Cookie name used for admin impersonation of another user.
pub const IMPERSONATE_COOKIE: &str = "impersonate_user_id";

/// Extract the impersonate_user_id cookie value from request headers.
fn get_impersonate_cookie(request: &Request<Body>) -> Option<Uuid> {
    let cookie_header = request.headers().get("cookie")?.to_str().ok()?;
    let target_id_str = cookie_header
        .split(';')
        .map(|s| s.trim())
        .find_map(|s| s.strip_prefix("impersonate_user_id="))?;
    target_id_str.parse().ok()
}

/// If the admin user has an impersonation cookie, load the target user's context instead.
/// Returns the original user unchanged if no impersonation is active or if the target is invalid.
async fn try_impersonate(
    pool: &sqlx::PgPool,
    admin_user: WebUser,
    target_id: Option<Uuid>,
) -> WebUser {
    let target_id = match target_id {
        Some(id) if id != admin_user.id => id,
        _ => return admin_user,
    };

    let admin_id = admin_user.id;

    // Load the target user
    let target = sqlx::query_as::<_, (Uuid, String, String, bool)>(
        "SELECT id, email, name, is_admin FROM users WHERE id = $1",
    )
    .bind(target_id)
    .fetch_optional(pool)
    .await;

    match target {
        Ok(Some(user)) => {
            let org_memberships = sqlx::query_as::<_, (Uuid, String)>(
                "SELECT organization_id, role FROM organization_members WHERE user_id = $1",
            )
            .bind(user.0)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(org_id, role)| OrgMembership { org_id, role })
            .collect();

            WebUser {
                id: user.0,
                email: user.1,
                name: user.2,
                is_admin: user.3,
                org_memberships,
                impersonating_from: Some(admin_id),
            }
        }
        _ => admin_user,
    }
}

/// Middleware that enforces authentication and allowed_emails on all non-auth, non-asset routes.
///
/// axum-oidc-client's AuthLayer handles /auth, /auth/callback, /auth/logout and sets the
/// session cookie, but does not block unauthenticated requests on other routes.
/// This middleware reads the session from the cache, decodes the ID token to extract the
/// email, checks against the allowed_emails list, upserts the user record, and injects
/// `WebUser` into request extensions.
pub async fn require_auth(
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Pass through auth routes and static assets
    if path.starts_with("/auth")
        || path.starts_with("/assets/")
        || path.starts_with("/public/")
        || path == "/favicon.ico"
    {
        return next.run(request).await;
    }

    // DEV_ONLY_NO_AUTH: skip OIDC, look up the dev admin user (created at startup).
    if std::env::var("DEV_ONLY_NO_AUTH").as_deref() == Ok("1") {
        if let Ok(pool) = crate::server_pool() {
            let result = sqlx::query_as::<_, (Uuid, String, String, bool)>(
                "SELECT id, email, name, is_admin FROM users WHERE email = 'dev@localhost'",
            )
            .fetch_optional(&pool)
            .await;

            match result {
                Ok(Some(user)) => {
                    let org_memberships = sqlx::query_as::<_, (Uuid, String)>(
                        "SELECT organization_id, role FROM organization_members WHERE user_id = $1",
                    )
                    .bind(user.0)
                    .fetch_all(&pool)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(org_id, role)| OrgMembership { org_id, role })
                    .collect();

                    let mut web_user = WebUser {
                        id: user.0,
                        email: user.1,
                        name: user.2,
                        is_admin: user.3,
                        org_memberships,
                        impersonating_from: None,
                    };
                    // Impersonation support in dev mode too
                    let imp_id = get_impersonate_cookie(&request);
                    web_user = try_impersonate(&pool, web_user, imp_id).await;
                    request.extensions_mut().insert(web_user);
                }
                Ok(None) => {
                    tracing::error!("DEV_ONLY_NO_AUTH: dev user not found — was it created at startup?");
                }
                Err(e) => {
                    tracing::error!("DEV_ONLY_NO_AUTH: failed to look up dev user: {e}");
                }
            }
        }
        return next.run(request).await;
    }

    // Extract session ID from the private cookie jar
    let configuration = request
        .extensions()
        .get::<Arc<OAuthConfiguration>>()
        .cloned();
    let cache = request
        .extensions()
        .get::<Arc<dyn AuthCache + Send + Sync>>()
        .cloned();

    if let (Some(conf), Some(cache)) = (configuration, cache) {
        use axum_extra::extract::cookie::PrivateCookieJar;
        let jar = PrivateCookieJar::from_headers(
            request.headers(),
            conf.private_cookie_key.clone(),
        );

        if let Some(session_cookie) = jar.get(SESSION_KEY) {
            let session_id = session_cookie.value().to_string();
            if let Ok(Some(session)) = AuthCache::get_auth_session(cache.as_ref(), &session_id).await {
                if let Some(email) = email_from_id_token(&session.id_token) {
                    let oidc = config::config().oidc.as_ref();
                    let domain_ok = oidc.is_some_and(|o| o.allowed_domains.iter().any(|d| email.ends_with(&format!("@{d}"))));
                    let email_ok = oidc.is_some_and(|o| o.allowed_emails.contains(&email));
                    if domain_ok || email_ok {
                        // Resolve user from database and inject into extensions
                        let pool = crate::server_pool();
                        if let Ok(pool) = pool {
                            let display_name = name_from_id_token(&session.id_token);
                            match resolve_user(&pool, &email, display_name.as_deref()).await {
                                Ok(mut web_user) => {
                                    // Impersonation: if admin, check for impersonate cookie
                                    if web_user.is_admin {
                                        let imp_id = get_impersonate_cookie(&request);
                                        web_user = try_impersonate(&pool, web_user, imp_id).await;
                                    }
                                    request.extensions_mut().insert(web_user);
                                }
                                Err(e) => {
                                    tracing::error!("failed to resolve user {email}: {e}");
                                }
                            }
                        }
                        return next.run(request).await;
                    }
                    tracing::warn!("access denied for {email}");
                }
            }
        }
    }

    Redirect::to("/auth").into_response()
}
