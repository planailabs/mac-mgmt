//! OIDC authentication middleware, login/logout handlers, and session management.

use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_oidc_client::{
    auth::{AuthLayer, OAuthConfiguration, SESSION_KEY},
    auth_builder::OAuthConfigurationBuilder,
    auth_cache::AuthCache,
    cache::{TwoTierAuthCache, config::TwoTierCacheConfig},
    logout::handle_default_logout::DefaultLogoutHandler,
    sql_cache::{SqlAuthCache, SqlCacheConfig},
};
use serde::Deserialize;
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

use crate::config::AuthConfig;
use crate::types::{OrgMembership, WebUser};

/// Provider metadata stored at startup for use by `require_auth` and the login page.
pub struct ProviderMeta {
    pub slug: String,
    pub name: String,
    pub issuer: Option<String>,
    pub allow_all: bool,
    pub allowed_domains: Vec<String>,
    pub allowed_emails: Vec<String>,
    pub auto_join_orgs: Vec<String>,
    /// Emails granted admin, from the app's AuthConfig. Passed to the resolver
    /// so admin designation actually takes effect on login.
    pub admin_emails: Vec<String>,
}

/// Providers populated during `build_auth_layers()`.
pub static AUTH_PROVIDERS: OnceLock<Vec<ProviderMeta>> = OnceLock::new();

/// Global user resolver, set at startup.
static USER_RESOLVER: OnceLock<Arc<dyn UserResolver>> = OnceLock::new();

/// Trait for app-specific user resolution from OIDC claims.
///
/// Each application implements this to upsert users into its own database,
/// auto-join organizations, and load memberships.
#[async_trait::async_trait]
pub trait UserResolver: Send + Sync + 'static {
    /// Resolve (upsert) a user from their email and display name.
    ///
    /// Called on every authenticated request. Must be idempotent.
    /// `admin_emails` contains the list of emails configured as admins.
    /// `auto_join_orgs` contains the org names to auto-add the user to.
    async fn resolve_user(
        &self,
        email: &str,
        name: Option<&str>,
        admin_emails: &[String],
        auto_join_orgs: &[String],
    ) -> Result<WebUser, anyhow::Error>;

    /// Load a user by ID (for impersonation).
    async fn load_user_by_id(&self, id: Uuid) -> Result<Option<WebUser>, anyhow::Error>;
}

/// Install the global user resolver. Must be called before serving requests.
pub fn set_user_resolver(resolver: Arc<dyn UserResolver>) {
    let _ = USER_RESOLVER.set(resolver);
}

fn get_resolver() -> Option<&'static Arc<dyn UserResolver>> {
    USER_RESOLVER.get()
}

// ── JWT helpers ──────────────────────────────────────────────────────────

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

fn email_from_id_token(id_token: &str) -> Option<String> {
    jwt_claims(id_token)?
        .get("email")?
        .as_str()
        .map(String::from)
}

fn name_from_id_token(id_token: &str) -> Option<String> {
    jwt_claims(id_token)?
        .get("name")?
        .as_str()
        .map(String::from)
}

fn issuer_from_id_token(id_token: &str) -> Option<String> {
    jwt_claims(id_token)?.get("iss")?.as_str().map(String::from)
}

fn provider_for_issuer(iss: &str) -> Option<&'static ProviderMeta> {
    let providers = AUTH_PROVIDERS.get()?;
    let iss_norm = iss.trim_end_matches('/');
    providers.iter().find(|p| {
        p.issuer
            .as_deref()
            .is_some_and(|i| i.trim_end_matches('/') == iss_norm)
    })
}

// ── Build auth layers ────────────────────────────────────────────────────

/// Build OIDC AuthLayers (one per provider) and a shared session cache.
pub async fn build_auth_layers(
    auth: &AuthConfig,
    db_url: &str,
) -> (Vec<AuthLayer>, Arc<dyn AuthCache + Send + Sync>) {
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

    let base_url = auth.external_url.trim_end_matches('/').to_string();
    let logout_handler = Arc::new(DefaultLogoutHandler);
    let mut layers = Vec::new();
    let mut metas = Vec::new();

    for provider in &auth.providers {
        if provider.allow_all
            && (!provider.allowed_domains.is_empty() || !provider.allowed_emails.is_empty())
        {
            panic!(
                "provider {}: allow_all is mutually exclusive with allowed_domains/allowed_emails",
                provider.slug
            );
        }

        let base_path = format!("/auth/{}", provider.slug);
        let redirect_uri = format!("{base_url}/auth/{}/callback", provider.slug);

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
            allow_all: provider.allow_all,
            allowed_domains: provider.allowed_domains.clone(),
            allowed_emails: provider.allowed_emails.clone(),
            auto_join_orgs: provider.auto_join_orgs.clone(),
            admin_emails: auth.admin_emails.clone(),
        });
    }

    let _ = AUTH_PROVIDERS.set(metas);

    (layers, cache)
}

// ── Impersonation helpers ────────────────────────────────────────────────

pub const IMPERSONATE_COOKIE: &str = "impersonate_user_id";

fn get_impersonate_cookie(request: &Request<Body>) -> Option<Uuid> {
    let cookie_header = request.headers().get("cookie")?.to_str().ok()?;
    let target_id_str = cookie_header
        .split(';')
        .map(|s| s.trim())
        .find_map(|s| s.strip_prefix("impersonate_user_id="))?;
    match target_id_str.parse() {
        Ok(id) => Some(id),
        Err(_) => {
            tracing::warn!(raw = target_id_str, "impersonation: invalid UUID in cookie");
            None
        }
    }
}

async fn try_impersonate(admin_user: WebUser, target_id: Option<Uuid>) -> WebUser {
    let target_id = match target_id {
        Some(id) if id != admin_user.id => id,
        _ => return admin_user,
    };

    let admin_id = admin_user.id;
    let resolver = match get_resolver() {
        Some(r) => r,
        None => {
            tracing::warn!("impersonation: no user resolver set");
            return admin_user;
        }
    };

    match resolver.load_user_by_id(target_id).await {
        Ok(Some(mut target)) => {
            tracing::info!(
                admin = %admin_id,
                target = %target_id,
                target_email = %target.email,
                "impersonating user"
            );
            target.impersonating_from = Some(admin_id);
            target
        }
        Ok(None) => {
            tracing::warn!(target = %target_id, "impersonation: target user not found");
            admin_user
        }
        Err(e) => {
            tracing::error!(target = %target_id, error = %e, "impersonation: failed to load target user");
            admin_user
        }
    }
}

// ── Login redirect ───────────────────────────────────────────────────────

fn login_redirect(return_to: Option<&str>) -> Redirect {
    let providers = AUTH_PROVIDERS.get();
    let redirect_param = return_to
        .filter(|p| p.starts_with('/') && !p.starts_with("//"))
        .map(|p| {
            format!(
                "?redirect={}",
                percent_encoding::utf8_percent_encode(p, percent_encoding::NON_ALPHANUMERIC)
            )
        });
    match providers.map(|p| p.as_slice()) {
        Some([only]) => {
            let base = format!("/auth/{}", only.slug);
            match &redirect_param {
                Some(q) => Redirect::to(&format!("{base}{q}")),
                None => Redirect::to(&base),
            }
        }
        _ => match &redirect_param {
            Some(q) => Redirect::to(&format!("/auth/login{q}")),
            None => Redirect::to("/auth/login"),
        },
    }
}

// ── require_auth middleware ───────────────────────────────────────────────

/// Middleware that enforces authentication on all non-auth, non-asset routes.
///
/// Extracts the OIDC session, resolves the user via the `UserResolver` trait,
/// and injects `WebUser` into request extensions.
pub async fn require_auth(mut request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path();

    // Pass through auth routes and static assets.
    if path.starts_with("/auth")
        || path.starts_with("/assets/")
        || path.starts_with("/public/")
        || path == "/favicon.ico"
    {
        return next.run(request).await;
    }

    // DEV_ONLY_NO_AUTH bypass: look up dev user via resolver. Gated behind
    // `debug_assertions` so it is compiled out of release binaries entirely —
    // a stray env var in production cannot disable authentication. On resolver
    // failure we fall through to the real OIDC flow rather than serving the
    // request unauthenticated (fail closed, not open).
    #[cfg(debug_assertions)]
    if std::env::var("DEV_ONLY_NO_AUTH").as_deref() == Ok("1") {
        if let Some(resolver) = get_resolver() {
            match resolver
                .resolve_user("dev@localhost", Some("Dev Admin"), &[], &[])
                .await
            {
                Ok(mut web_user) => {
                    if web_user.is_admin {
                        let imp_id = get_impersonate_cookie(&request);
                        web_user = try_impersonate(web_user, imp_id).await;
                    }
                    request.extensions_mut().insert(web_user);
                    return next.run(request).await;
                }
                Err(e) => {
                    tracing::error!(
                        "DEV_ONLY_NO_AUTH: failed to resolve dev user, falling through to OIDC: {e}"
                    );
                }
            }
        }
    }

    // Extract session from the private cookie jar.
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
                    let issuer = issuer_from_id_token(&session.id_token);
                    let provider = issuer.as_deref().and_then(provider_for_issuer);

                    let (allowed, auto_join_orgs) = if let Some(p) = provider {
                        let domain_ok = p
                            .allowed_domains
                            .iter()
                            .any(|d| email.ends_with(&format!("@{d}")));
                        let email_ok = p.allowed_emails.contains(&email);
                        (
                            p.allow_all || domain_ok || email_ok,
                            p.auto_join_orgs.as_slice(),
                        )
                    } else {
                        (false, [].as_slice())
                    };

                    if allowed {
                        if let Some(resolver) = get_resolver() {
                            let display_name = name_from_id_token(&session.id_token);
                            let admin_emails: &[String] =
                                provider.map(|p| p.admin_emails.as_slice()).unwrap_or(&[]);
                            match resolver
                                .resolve_user(
                                    &email,
                                    display_name.as_deref(),
                                    admin_emails,
                                    auto_join_orgs,
                                )
                                .await
                            {
                                Ok(mut web_user) => {
                                    if web_user.is_admin {
                                        let imp_id = get_impersonate_cookie(&request);
                                        web_user = try_impersonate(web_user, imp_id).await;
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

    let return_to = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string());
    login_redirect(return_to.as_deref()).into_response()
}

// ── Login page ───────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct LoginQuery {
    #[serde(default)]
    redirect: Option<String>,
}

/// Login page shown when multiple OIDC providers are configured.
pub async fn login_page(
    axum::extract::Query(query): axum::extract::Query<LoginQuery>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let lang = plan_ai_html::Lang::from_accept_language(
        headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    let providers = AUTH_PROVIDERS.get().map(|p| p.as_slice()).unwrap_or(&[]);

    let redirect_suffix = query
        .redirect
        .as_deref()
        .filter(|p| p.starts_with('/') && !p.starts_with("//"))
        .map(|p| {
            format!(
                "?redirect={}",
                percent_encoding::utf8_percent_encode(p, percent_encoding::NON_ALPHANUMERIC)
            )
        })
        .unwrap_or_default();

    if let [only] = providers {
        return Redirect::to(&format!("/auth/{}{redirect_suffix}", only.slug)).into_response();
    }

    let buttons: String = providers
        .iter()
        .map(|p| {
            format!(
                r#"<a href="/auth/{slug}{redirect_suffix}" class="btn btn-secondary btn-lg" style="display:flex;margin-top:.5rem">{name}</a>"#,
                slug = p.slug,
                name = plan_ai_html::escape(&p.name),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let title = plan_ai_html::tr(lang, "sign-in");
    let body = format!(r#"<h1 class="h-page">{title}</h1>{buttons}"#);
    Html(plan_ai_html::Page::new(&title, body).lang(lang).render()).into_response()
}

// ── Logout handler ───────────────────────────────────────────────────────

/// Generic logout handler: clears session and redirects to login page.
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

        if let Some(session_cookie) = jar.get(SESSION_KEY) {
            let _ = cache.invalidate_auth_session(session_cookie.value()).await;
        }

        let jar = jar.remove(
            Cookie::build(SESSION_KEY)
                .path("/")
                .http_only(true)
                .same_site(axum_extra::extract::cookie::SameSite::Strict)
                .secure(true),
        );

        return (jar, login_redirect(None)).into_response();
    }

    login_redirect(None).into_response()
}
