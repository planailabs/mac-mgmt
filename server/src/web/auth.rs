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

use crate::config;

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

/// Build the OIDC AuthLayer and session cache from config.
pub async fn build_auth_layer(
    db_url: &str,
) -> (AuthLayer, Arc<dyn AuthCache + Send + Sync>) {
    let cfg = &config::config().oidc;

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
        .with_session_max_age(480) // 8 hours in minutes
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

/// Middleware that enforces authentication and allowed_emails on all non-auth, non-asset routes.
///
/// axum-oidc-client's AuthLayer handles /auth, /auth/callback, /auth/logout and sets the
/// session cookie, but does not block unauthenticated requests on other routes.
/// This middleware reads the session from the cache, decodes the ID token to extract the
/// email, and checks against the allowed_emails list.
pub async fn require_auth(
    request: Request<Body>,
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
                    let oidc = &config::config().oidc;
                    let domain_ok = oidc.allowed_domains.iter().any(|d| email.ends_with(&format!("@{d}")));
                    let email_ok = oidc.allowed_emails.contains(&email);
                    if domain_ok || email_ok {
                        return next.run(request).await;
                    }
                    tracing::warn!("access denied for {email}");
                }
            }
        }
    }

    Redirect::to("/auth").into_response()
}
