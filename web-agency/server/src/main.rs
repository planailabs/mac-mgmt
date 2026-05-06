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

    // Mark any deployments left in-flight from a previous run as failed.
    api::deploy::recover_interrupted_deployments(&pool).await;

    // Start periodic background sync (DNS records, domain expiry).
    api::sync::spawn(pool.clone());

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
        if let Err(e) = api::acme::issue_cert(pool, domain).await {
            tracing::error!(domain, "renewal failed: {e}");
        }
    }

    // Issue certs for domains with webspace bindings but no cert row
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT CASE \
             WHEN s.name IS NOT NULL AND s.name != '@' THEN s.name || '.' || d.name \
             ELSE d.name \
         END AS hostname \
         FROM webspace_domains wd \
         JOIN webspaces w ON w.id = wd.webspace_id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE w.hosting_type = 'local' AND w.local_port IS NOT NULL \
           AND NOT EXISTS (SELECT 1 FROM certificates c WHERE c.domain = \
               CASE WHEN s.name IS NOT NULL AND s.name != '@' THEN s.name || '.' || d.name ELSE d.name END)",
    )
    .fetch_all(pool)
    .await?;

    for domain in &missing {
        tracing::info!(domain, "issuing cert for new domain");
        if let Err(e) = api::acme::issue_cert(pool, domain).await {
            tracing::error!(domain, "issuance failed: {e}");
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
        use std::sync::{Arc, OnceLock};
        use std::sync::atomic::AtomicUsize;

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

            let mut router = axum::Router::new()
                .serve_dioxus_application(ServeConfig::new(), web::app::App);

            // Mount deploy API (uses its own Bearer token auth, not OIDC)
            let active_deploys = ACTIVE_DEPLOYS
                .get_or_init(|| Arc::new(AtomicUsize::new(0)))
                .clone();
            let deploy_router = crate::api::deploy::router(crate::api::deploy::DeployState {
                pool: crate::server_pool().expect("pool for deploy API"),
                active_deploys,
            });
            router = router.merge(deploy_router);

            // Mount internal API (used by the reverse proxy)
            {
                let (reload_tx, _) = tokio::sync::broadcast::channel::<()>(16);
                let internal_router =
                    crate::api::internal::router(crate::api::internal::InternalState {
                        pool: crate::server_pool().expect("pool for internal API"),
                        reload_tx: Arc::new(reload_tx),
                    });
                router = router.merge(internal_router);
            }

            if let Some(auth_layers) = auth_layers {
                router = router
                    .route("/auth/login", axum::routing::get(plan_ai_auth::login_page))
                    .route("/auth/logout", axum::routing::get(plan_ai_auth::logout_handler))
                    .layer(axum::middleware::from_fn(plan_ai_auth::require_auth));
                for layer in auth_layers {
                    router = router.layer(layer);
                }
            } else if dev_no_auth {
                router = router
                    .layer(axum::middleware::from_fn(plan_ai_auth::require_auth));
            }

            Ok(router)
        });
    }

    #[cfg(all(not(feature = "server"), feature = "webui"))]
    {
        dioxus::launch(web::app::App);
    }
}
