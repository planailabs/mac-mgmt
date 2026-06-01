#[cfg(feature = "server")]
mod api;
#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
pub mod credentials;
#[cfg(feature = "server")]
mod crypto;
#[cfg(feature = "server")]
mod db;
#[cfg(feature = "server")]
mod local_hosting;
#[cfg(feature = "webui")]
mod web;

#[cfg(all(feature = "server", feature = "webui"))]
mod server_state {
    use sqlx::PgPool;
    use std::sync::OnceLock;

    static POOL: OnceLock<PgPool> = OnceLock::new();

    pub fn set_pool(pool: PgPool) {
        POOL.set(pool).expect("pool already initialized");
    }

    pub fn server_pool() -> Result<PgPool, dioxus::prelude::ServerFnError> {
        POOL.get()
            .cloned()
            .ok_or_else(|| dioxus::prelude::ServerFnError::new("database pool not initialized"))
    }
}

#[cfg(all(feature = "server", feature = "webui"))]
pub fn server_pool() -> Result<sqlx::PgPool, dioxus::prelude::ServerFnError> {
    server_state::server_pool()
}

#[cfg(feature = "server")]
async fn init_server() -> sqlx::PgPool {
    let cfg = config::load();
    let pool = db::connect(&cfg.database.url).await;

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run migrations");

    // Ensure the internal API token file exists (for proxy ↔ server auth).
    if let Some(proxy_cfg) = &cfg.proxy {
        let path = std::path::Path::new(&proxy_cfg.internal_token_path);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            use rand::Rng;
            let token_bytes: [u8; 32] = rand::rng().random();
            let token = hex::encode(token_bytes);
            match std::fs::write(path, &token) {
                Ok(()) => tracing::info!(path = %path.display(), "generated internal API token"),
                Err(e) => {
                    tracing::error!(path = %path.display(), "failed to write internal token: {e}")
                }
            }
        }
    }

    // Ensure a content directory exists for every local webspace.
    if let Err(e) = local_hosting::ensure_webspace_dirs(&pool).await {
        tracing::error!("failed to ensure local webspace dirs: {e}");
    }

    // Mark any deployments left in-flight from a previous run as failed.
    api::deploy::recover_interrupted_deployments(&pool).await;

    // Start periodic background sync (DNS records, domain expiry).
    api::sync::spawn(pool.clone());

    // Start periodic reachability checks (every 15 minutes).
    api::reachability::spawn(pool.clone());

    // Start periodic cert renewal (hourly: renew expiring, issue missing).
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            // Wait a bit before first check to let the server fully start.
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            loop {
                if let Err(e) = cert_renewal_tick(&pool).await {
                    tracing::error!("cert renewal tick failed: {e}");
                }
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });
    }

    #[cfg(feature = "webui")]
    server_state::set_pool(pool.clone());

    // Install the shared auth user resolver.
    let admin_emails = cfg
        .auth
        .as_ref()
        .map(|a| a.admin_emails.clone())
        .unwrap_or_default();
    let resolver = web::auth::PgUserResolver::new(pool.clone(), admin_emails);
    plan_ai_auth::set_user_resolver(resolver);

    pool
}

#[cfg(feature = "server")]
async fn cert_renewal_tick(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    use std::sync::atomic::Ordering;
    let counters = &api::counters::COUNTERS;
    counters.reset_cert_gauges();

    // Renew certs expiring within 30 days
    let expiring: Vec<String> = sqlx::query_scalar(
        "SELECT domain FROM certificates \
         WHERE not_after < now() + interval '30 days' \
           AND issuer != 'self-signed' \
           AND acme_status != 'pending'",
    )
    .fetch_all(pool)
    .await?;

    for domain in &expiring {
        tracing::info!(domain, "renewing expiring cert");
        match api::acme::issue_cert(pool, domain).await {
            Ok(()) => {
                counters
                    .cert_renewal_success
                    .fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::error!(domain, "renewal failed: {e}");
                counters.cert_renewal_failed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    // Issue certs for proxy hosts with domain bindings but no cert row.
    // (Cloudflare hosts terminate TLS at Cloudflare, so they need no local cert.)
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT CASE \
             WHEN s.name IS NOT NULL AND s.name != '@' THEN s.name || '.' || d.name \
             ELSE d.name \
         END AS hostname \
         FROM webspace_host_domains whd \
         JOIN webspace_hosts h ON h.id = whd.webspace_host_id AND h.kind = 'proxy' \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE NOT EXISTS (SELECT 1 FROM certificates c WHERE c.domain = \
               CASE WHEN s.name IS NOT NULL AND s.name != '@' THEN s.name || '.' || d.name ELSE d.name END)",
    )
    .fetch_all(pool)
    .await?;

    for domain in &missing {
        tracing::info!(domain, "issuing cert for new domain");
        match api::acme::issue_cert(pool, domain).await {
            Ok(()) => {
                counters
                    .cert_issuance_success
                    .fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::error!(domain, "issuance failed: {e}");
                counters
                    .cert_issuance_failed
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    if !expiring.is_empty() || !missing.is_empty() {
        tracing::info!(
            renewed = expiring.len(),
            issued = missing.len(),
            "cert renewal tick complete"
        );
    }

    Ok(())
}

fn main() {
    #[cfg(feature = "server")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    #[cfg(all(feature = "server", feature = "webui"))]
    {
        use tracing_subscriber::EnvFilter;
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
            .init();
    }

    #[cfg(all(feature = "server", feature = "webui"))]
    {
        use dioxus::server::{DioxusRouterExt, ServeConfig, axum};
        use std::sync::atomic::AtomicUsize;
        use std::sync::{Arc, OnceLock};

        static INIT: OnceLock<Option<Vec<plan_ai_auth::AuthLayer>>> = OnceLock::new();
        static ACTIVE_DEPLOYS: OnceLock<Arc<AtomicUsize>> = OnceLock::new();

        if std::env::var("PORT").is_err() {
            let cfg = config::load();
            unsafe { std::env::set_var("PORT", cfg.web.port.to_string()) };
        }

        // SIGTERM handler: drain active deployments before exit.
        std::thread::spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("failed to register SIGTERM handler")
                        .recv()
                        .await;
                    tracing::info!("received SIGTERM, draining deployments...");
                    if let Some(active) = ACTIVE_DEPLOYS.get() {
                        crate::api::deploy::drain_active_deploys(
                            active,
                            std::time::Duration::from_secs(120),
                        )
                        .await;
                    }
                    tracing::info!("shutdown complete");
                    std::process::exit(0);
                });
        });

        dioxus::serve(move || async move {
            let dev_no_auth = std::env::var("DEV_ONLY_NO_AUTH").as_deref() == Ok("1");
            let auth_layers = if let Some(layers) = INIT.get() {
                layers.clone()
            } else {
                let _pool = init_server().await;
                let cfg = config::load();

                let auth_layers = if dev_no_auth {
                    tracing::warn!("DEV_ONLY_NO_AUTH=1 — OIDC disabled, using dev admin user");
                    if let Ok(pool) = crate::server_pool() {
                        if let Err(e) = sqlx::query(
                            "INSERT INTO users (email, name, is_admin) VALUES ('dev@localhost', 'Dev Admin', true) \
                             ON CONFLICT (email) DO UPDATE SET is_admin = true",
                        )
                        .execute(&pool)
                        .await {
                            tracing::error!("failed to create dev user: {e}");
                        }
                    }
                    None
                } else if let Some(auth) = &cfg.auth {
                    let (layers, _cache) =
                        plan_ai_auth::build_auth_layers(auth, &cfg.database.url).await;
                    Some(layers)
                } else {
                    tracing::warn!("[auth] not configured — web authentication disabled");
                    None
                };

                let _ = INIT.set(auth_layers.clone());
                auth_layers
            };

            // Web UI (Dioxus app + auth-gated routes). require_auth and the
            // OIDC session layers attach only here; the deploy and internal
            // routers do their own Bearer-token check and must NOT be
            // wrapped, otherwise unauthenticated requests get redirected to
            // /auth/login before their handler-level auth runs.
            let mut web_router =
                axum::Router::new().serve_dioxus_application(ServeConfig::new(), web::app::App);

            if let Some(auth_layers) = auth_layers {
                web_router = web_router
                    .route("/auth/login", axum::routing::get(plan_ai_auth::login_page))
                    .route(
                        "/auth/logout",
                        axum::routing::get(plan_ai_auth::logout_handler),
                    )
                    .route(
                        "/proxy-gate",
                        axum::routing::get(crate::api::internal::proxy_gate),
                    )
                    .layer(axum::middleware::from_fn(plan_ai_auth::require_auth));
                for layer in auth_layers {
                    web_router = web_router.layer(layer);
                }
            } else if dev_no_auth {
                web_router = web_router
                    .route(
                        "/proxy-gate",
                        axum::routing::get(crate::api::internal::proxy_gate),
                    )
                    .layer(axum::middleware::from_fn(plan_ai_auth::require_auth));
            }

            // Mount deploy API (uses its own Bearer token auth, not OIDC)
            let active_deploys = ACTIVE_DEPLOYS
                .get_or_init(|| Arc::new(AtomicUsize::new(0)))
                .clone();
            let deploy_router = crate::api::deploy::router(crate::api::deploy::DeployState {
                pool: crate::server_pool().expect("pool for deploy API"),
                active_deploys,
            });

            // Mount internal API (used by the reverse proxy)
            let internal_router = {
                let (reload_tx, _) = tokio::sync::broadcast::channel::<()>(16);
                let reload_tx = Arc::new(reload_tx);
                crate::api::internal::set_reload_tx(reload_tx.clone());
                crate::api::internal::router(crate::api::internal::InternalState {
                    pool: crate::server_pool().expect("pool for internal API"),
                    reload_tx,
                })
            };

            // Mount metrics API (Bearer token auth, not OIDC)
            let metrics_router = crate::api::metrics::router(crate::api::metrics::MetricsState {
                pool: crate::server_pool().expect("pool for metrics API"),
            });

            // Mount changedetection webhook API (secret-in-URL auth)
            let cd_router = crate::api::changedetection::router(
                crate::api::changedetection::ChangeDetectionState {
                    pool: crate::server_pool().expect("pool for changedetection API"),
                },
            );

            // The agency app router (Dioxus + APIs, with their own auth).
            let agency_router = axum::Router::new()
                .merge(deploy_router)
                .merge(internal_router)
                .merge(metrics_router)
                .merge(cd_router)
                .merge(web_router);

            // A fully separate router for proxy-forwarded static webspace
            // requests (no agency auth/Dioxus middleware).
            let webspace_router = crate::api::static_serve::router();

            // Steer by the webspace header: requests carrying it go to the
            // webspace router, everything else to the agency router. Expressed
            // as a fallback service (not a wrapping layer) so the two routers
            // stay fully independent.
            use tower::ServiceExt; // oneshot
            let dispatch = tower::service_fn(move |req: axum::extract::Request| {
                let agency = agency_router.clone();
                let webspace = webspace_router.clone();
                async move {
                    if req
                        .headers()
                        .contains_key(crate::api::static_serve::WEBSPACE_HEADER)
                    {
                        webspace.oneshot(req).await
                    } else {
                        agency.oneshot(req).await
                    }
                }
            });

            Ok(axum::Router::new().fallback_service(dispatch))
        });
    }

    #[cfg(all(not(feature = "server"), feature = "webui"))]
    {
        dioxus::launch(web::app::App);
    }
}
