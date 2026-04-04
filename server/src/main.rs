#[cfg(any(feature = "server", feature = "server-api-only"))]
mod api;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod config;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod db;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod mcp_schema;
#[cfg(feature = "webui")]
mod anthropic;
#[cfg(feature = "webui")]
mod models;
#[cfg(feature = "webui")]
mod web;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod xzar;

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

#[cfg(any(feature = "server", feature = "server-api-only"))]
async fn init_server() -> (sqlx::PgPool, rocket::Rocket<rocket::Ignite>) {
    let cfg = config::load();
    let pool = db::connect(&cfg.database.url).await;

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run migrations");

    #[cfg(feature = "webui")]
    server_state::set_pool(pool.clone());

    let api_rocket = api::build_rocket(pool.clone(), cfg.api.port)
        .ignite()
        .await
        .expect("failed to ignite API rocket");

    (pool, api_rocket)
}

fn main() {
    #[cfg(all(feature = "server", feature = "webui"))]
    {
        use dioxus::server::{DioxusRouterExt, ServeConfig, axum};
        use std::sync::OnceLock;

        static INIT: OnceLock<Option<axum_oidc_client::auth::AuthLayer>> = OnceLock::new();

        // Set PORT env var for dioxus if not already set.
        // SAFETY: called before any threads are spawned.
        if std::env::var("PORT").is_err() {
            let cfg = config::load();
            unsafe { std::env::set_var("PORT", cfg.web.port.to_string()) };
        }

        // Dioxus's release-mode server doesn't handle SIGTERM. We register
        // the handler before any tokio runtime is created so it's available
        // on a dedicated thread.  On SIGTERM we:
        //   1. Tell Rocket to shut down gracefully (drains in-flight requests)
        //   2. Exit the process so Dioxus/axum stops too
        static ROCKET_SHUTDOWN: std::sync::OnceLock<rocket::Shutdown> = std::sync::OnceLock::new();

        std::thread::spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("failed to register SIGTERM handler")
                        .recv()
                        .await;
                    tracing::info!("received SIGTERM, shutting down");
                    if let Some(handle) = ROCKET_SHUTDOWN.get() {
                        handle.clone().notify();
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                    std::process::exit(0);
                });
        });

        dioxus::serve(move || async move {
            let auth_layer = if let Some(layer) = INIT.get() {
                layer.clone()
            } else {
                let (_pool, api_rocket) = init_server().await;
                let cfg = config::load();

                let auth_layer = if cfg.oidc.is_some() {
                    let (layer, _cache) =
                        web::auth::build_auth_layer(&cfg.database.url).await;
                    Some(layer)
                } else {
                    tracing::warn!("OIDC not configured — web authentication disabled");
                    None
                };

                let _ = ROCKET_SHUTDOWN.set(api_rocket.shutdown());
                tokio::spawn(async move {
                    if let Err(e) = api_rocket.launch().await {
                        tracing::error!("API server failed: {e}");
                    }
                });

                let _ = INIT.set(auth_layer.clone());
                auth_layer
            };

            let mut router = axum::Router::new()
                .serve_dioxus_application(ServeConfig::new(), web::app::App);

            if let Some(auth_layer) = auth_layer {
                router = router
                    .layer(axum::middleware::from_fn(web::auth::require_auth))
                    .layer(auth_layer);
            }

            Ok(router)
        });
    }

    // API-only mode: no web UI, just Rocket
    #[cfg(feature = "server-api-only")]
    {
        tracing_subscriber::fmt::init();
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async {
            let (_pool, api_rocket) = init_server().await;

            let shutdown = api_rocket.shutdown();
            tokio::spawn(async move {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler")
                    .recv()
                    .await;
                tracing::info!("received SIGTERM, shutting down");
                shutdown.notify();
            });

            if let Err(e) = api_rocket.launch().await {
                tracing::error!("API server failed: {e}");
            }
        });
    }

    #[cfg(not(any(feature = "server", feature = "server-api-only")))]
    {
        dioxus::launch(web::app::App);
    }
}
