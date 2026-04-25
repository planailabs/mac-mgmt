use axum_oidc_client::{
    auth::{AuthLayer, OAuthConfiguration, SESSION_KEY},
    auth_builder::OAuthConfigurationBuilder,
    auth_cache::AuthCache,
    cache::{TwoTierAuthCache, config::TwoTierCacheConfig},
    logout::handle_default_logout::DefaultLogoutHandler,
    sql_cache::{SqlAuthCache, SqlCacheConfig},
};
use dioxus::fullstack::axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

use super::user::{OrgMembership, WebUser};
use crate::config;

/// Provider metadata stored at startup for use by `require_auth` and the login page.
pub struct ProviderMeta {
    pub slug: String,
    pub name: String,
    pub issuer: Option<String>,
    pub allowed_domains: Vec<String>,
    pub allowed_emails: Vec<String>,
    pub auto_join_orgs: Vec<String>,
}

/// Providers populated during `build_auth_layers()`.
static AUTH_PROVIDERS: OnceLock<Vec<ProviderMeta>> = OnceLock::new();

/// Decode the JWT payload (base64url, no verification — already validated by the OIDC client).
fn jwt_claims(id_token: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload = engine.decode(parts[1]).ok()?;
    serde_json::from_slice(&payload).ok()
}

/// Extract the email from a JWT ID token's payload.
fn email_from_id_token(id_token: &str) -> Option<String> {
    jwt_claims(id_token)?.get("email")?.as_str().map(String::from)
}

/// Extract the user's display name from the JWT ID token payload.
fn name_from_id_token(id_token: &str) -> Option<String> {
    jwt_claims(id_token)?.get("name")?.as_str().map(String::from)
}

/// Extract the issuer (`iss`) claim from the JWT ID token payload.
fn issuer_from_id_token(id_token: &str) -> Option<String> {
    jwt_claims(id_token)?.get("iss")?.as_str().map(String::from)
}

/// Find the provider that issued this token by matching the JWT `iss` claim.
fn provider_for_issuer(iss: &str) -> Option<&'static ProviderMeta> {
    let providers = AUTH_PROVIDERS.get()?;
    // Normalise trailing slashes for comparison.
    let iss_norm = iss.trim_end_matches('/');
    providers.iter().find(|p| {
        p.issuer
            .as_deref()
            .is_some_and(|i| i.trim_end_matches('/') == iss_norm)
    })
}

/// Build OIDC AuthLayers (one per provider) and a shared session cache.
pub async fn build_auth_layers(
    db_url: &str,
) -> (Vec<AuthLayer>, Arc<dyn AuthCache + Send + Sync>) {
    let auth = config::config()
        .auth
        .as_ref()
        .expect("build_auth_layers called without [auth] config");

    // --- shared session cache ---------------------------------------------------
    let cache: Arc<dyn AuthCache + Send + Sync> = if let Some(redis_url) = &auth.redis_url {
        let redis_cache = axum_oidc_client::redis::AuthCache::new(redis_url, 28800);
        Arc::new(
            TwoTierAuthCache::new(Some(Arc::new(redis_cache)), TwoTierCacheConfig::default())
                .expect("failed to create two-tier cache with Redis"),
        )
    } else {
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
            TwoTierAuthCache::new(Some(Arc::new(sql_cache)), TwoTierCacheConfig::default())
                .expect("failed to create two-tier cache with PostgreSQL"),
        )
    };

    // --- per-provider auth layers -----------------------------------------------
    let web_port = config::config().web.port;
    let logout_handler = Arc::new(DefaultLogoutHandler);
    let mut layers = Vec::new();
    let mut metas = Vec::new();

    for provider in &auth.providers {
        let base_path = format!("/auth/{}", provider.slug);

        let redirect_uri = provider.redirect_uri.clone().unwrap_or_else(|| {
            format!("http://localhost:{web_port}/auth/{}/callback", provider.slug)
        });

        let mut builder = OAuthConfigurationBuilder::default();

        if let Some(issuer) = &provider.issuer {
            builder = builder
                .with_issuer(issuer)
                .await
                .unwrap_or_else(|e| panic!("OIDC discovery failed for {}: {e}", provider.slug));
        }

        let scopes: Vec<&str> = provider
            .scopes
            .as_ref()
            .map(|s| s.iter().map(|s| s.as_str()).collect())
            .unwrap_or_else(|| vec!["openid", "email", "profile"]);

        let oauth_config = builder
            .with_client_id(&provider.client_id)
            .with_client_secret(&provider.client_secret)
            .with_redirect_uri(&redirect_uri)
            .with_private_cookie_key(&auth.cookie_secret)
            .with_scopes(scopes)
            .with_base_path(&base_path)
            .with_post_logout_redirect_uri("/auth/login")
            // axum-oidc-client 0.3.0 units bug: value is treated as seconds
            // despite being documented as minutes. 21600 sec = 6h sliding window.
            .with_session_max_age(21600)
            .build()
            .unwrap_or_else(|e| panic!("failed to build OIDC config for {}: {e}", provider.slug));

        let layer = AuthLayer::new(
            Arc::new(oauth_config),
            cache.clone(),
            logout_handler.clone(),
        );
        layers.push(layer);

        metas.push(ProviderMeta {
            slug: provider.slug.clone(),
            name: provider.name.clone(),
            issuer: provider.issuer.clone(),
            allowed_domains: provider.allowed_domains.clone(),
            allowed_emails: provider.allowed_emails.clone(),
            auto_join_orgs: provider.auto_join_orgs.clone(),
        });
    }

    let _ = AUTH_PROVIDERS.set(metas);

    (layers, cache)
}

/// Upsert the user record, auto-join orgs, and load org memberships.
async fn resolve_user(
    pool: &sqlx::PgPool,
    email: &str,
    name: Option<&str>,
    auto_join_orgs: &[String],
) -> Result<WebUser, sqlx::Error> {
    let auth = config::config().auth.as_ref();
    let is_admin_email = auth.is_some_and(|a| a.admin_emails.contains(&email.to_string()));

    // Upsert user: create on first login, update name on subsequent logins.
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

    // Auto-join organizations for this provider (idempotent).
    for org_name in auto_join_orgs {
        let org_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM organizations WHERE name = $1",
        )
        .bind(org_name)
        .fetch_optional(pool)
        .await?;

        if let Some(org_id) = org_id {
            sqlx::query(
                "INSERT INTO organization_members (organization_id, user_id, role) \
                 VALUES ($1, $2, 'read') ON CONFLICT DO NOTHING",
            )
            .bind(org_id)
            .bind(user.0)
            .execute(pool)
            .await?;
        }
    }

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

/// Return the redirect target for unauthenticated requests.
fn login_redirect() -> Redirect {
    let providers = AUTH_PROVIDERS.get();
    match providers.map(|p| p.as_slice()) {
        Some([only]) => Redirect::to(&format!("/auth/{}", only.slug)),
        _ => Redirect::to("/auth/login"),
    }
}

/// Middleware that enforces authentication on all non-auth, non-asset routes.
///
/// For each authenticated session it extracts the JWT `iss` claim to determine
/// which provider issued the token, then checks only that provider's
/// `allowed_domains`/`allowed_emails`. The user is upserted and auto-joined to
/// the provider's `auto_join_orgs`.
pub async fn require_auth(mut request: Request<Body>, next: Next) -> Response {
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
                    tracing::error!(
                        "DEV_ONLY_NO_AUTH: dev user not found — was it created at startup?"
                    );
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
        let jar =
            PrivateCookieJar::from_headers(request.headers(), conf.private_cookie_key.clone());

        if let Some(session_cookie) = jar.get(SESSION_KEY) {
            let session_id = session_cookie.value().to_string();
            if let Ok(Some(session)) =
                AuthCache::get_auth_session(cache.as_ref(), &session_id).await
            {
                if let Some(email) = email_from_id_token(&session.id_token) {
                    // Determine which provider issued this token and check its
                    // allowed_domains / allowed_emails.
                    let issuer = issuer_from_id_token(&session.id_token);
                    let provider = issuer.as_deref().and_then(provider_for_issuer);

                    let (allowed, auto_join_orgs) = if let Some(p) = provider {
                        let domain_ok = p
                            .allowed_domains
                            .iter()
                            .any(|d| email.ends_with(&format!("@{d}")));
                        let email_ok = p.allowed_emails.contains(&email);
                        (domain_ok || email_ok, p.auto_join_orgs.as_slice())
                    } else {
                        (false, [].as_slice())
                    };

                    if allowed {
                        let pool = crate::server_pool();
                        if let Ok(pool) = pool {
                            let display_name = name_from_id_token(&session.id_token);
                            match resolve_user(
                                &pool,
                                &email,
                                display_name.as_deref(),
                                auto_join_orgs,
                            )
                            .await
                            {
                                Ok(mut web_user) => {
                                    if web_user.is_admin {
                                        let imp_id = get_impersonate_cookie(&request);
                                        web_user =
                                            try_impersonate(&pool, web_user, imp_id).await;
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
                    tracing::warn!(
                        "access denied for {email} (issuer: {})",
                        issuer.as_deref().unwrap_or("unknown")
                    );
                }
            }
        }
    }

    login_redirect().into_response()
}

/// Login page shown when multiple OIDC providers are configured.
pub async fn login_page() -> impl IntoResponse {
    let providers = AUTH_PROVIDERS.get().map(|p| p.as_slice()).unwrap_or(&[]);

    // Single provider: skip the page and redirect directly.
    if let [only] = providers {
        return Redirect::to(&format!("/auth/{}", only.slug)).into_response();
    }

    let buttons: String = providers
        .iter()
        .map(|p| {
            format!(
                r#"<a href="/auth/{slug}" class="login-btn">{name}</a>"#,
                slug = p.slug,
                name = p.name,
            )
        })
        .collect::<Vec<_>>()
        .join("\n            ");

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Sign in</title>
    <style>
        body {{ font-family: system-ui, -apple-system, sans-serif; background: #0f172a; color: #e2e8f0; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }}
        .container {{ text-align: center; max-width: 400px; width: 100%; padding: 2rem; }}
        h1 {{ font-size: 1.5rem; margin-bottom: 2rem; font-weight: 600; }}
        .login-btn {{ display: block; padding: 0.75rem 1.5rem; margin: 0.75rem 0; background: #1e293b; color: #e2e8f0; text-decoration: none; border-radius: 0.5rem; border: 1px solid #334155; font-size: 1rem; transition: background 0.15s; }}
        .login-btn:hover {{ background: #334155; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Sign in</h1>
        {buttons}
    </div>
</body>
</html>"#
    ))
    .into_response()
}

/// Generic logout handler: clears the session cookie and redirects to the login page.
/// Works regardless of which provider was used for login.
pub async fn logout_handler(request: Request<Body>) -> Response {
    let configuration = request
        .extensions()
        .get::<Arc<OAuthConfiguration>>()
        .cloned();
    let cache = request
        .extensions()
        .get::<Arc<dyn AuthCache + Send + Sync>>()
        .cloned();

    if let (Some(conf), Some(cache)) = (configuration, cache) {
        use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
        let jar =
            PrivateCookieJar::from_headers(request.headers(), conf.private_cookie_key.clone());

        // Invalidate session in cache
        if let Some(session_cookie) = jar.get(SESSION_KEY) {
            let _ = cache
                .invalidate_auth_session(session_cookie.value())
                .await;
        }

        // Remove the session cookie
        let jar = jar.remove(
            Cookie::build(SESSION_KEY)
                .path("/")
                .http_only(true)
                .same_site(axum_extra::extract::cookie::SameSite::Strict)
                .secure(true),
        );

        return (jar, login_redirect()).into_response();
    }

    login_redirect().into_response()
}
