#[cfg(feature = "server")]
mod api;
#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
mod db;
#[cfg(feature = "server")]
mod mcp_schema;
mod models;
mod web;
#[cfg(feature = "server")]
mod xzar;

#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
pub fn server_pool() -> Result<sqlx::PgPool, dioxus::prelude::ServerFnError> {
    server_state::server_pool()
}

fn main() {
    #[cfg(feature = "server")]
    {
        use dioxus::server::{DioxusRouterExt, ServeConfig, axum};
        use std::sync::OnceLock;

        // Shared state initialized once inside the first serve callback invocation.
        // serve() may call the callback multiple times (hot-reload), so we use OnceLock
        // to ensure one-time init.
        static INIT: OnceLock<Option<axum_oidc_client::auth::AuthLayer>> = OnceLock::new();
        let no_auth = std::env::var("DEV_ONLY_NO_AUTH").is_ok();

        // Set PORT env var for dioxus if not already set.
        // SAFETY: called before any threads are spawned.
        if std::env::var("PORT").is_err() {
            let cfg = config::load();
            unsafe { std::env::set_var("PORT", cfg.web.port.to_string()) };
        }

        dioxus::serve(move || async move {
            let auth_layer = if let Some(layer) = INIT.get() {
                layer.clone()
            } else {
                let cfg = config::load();
                let pool = db::connect(&cfg.database.url).await;

                sqlx::migrate!()
                    .run(&pool)
                    .await
                    .expect("failed to run migrations");

                server_state::set_pool(pool.clone());

                let auth_layer = if no_auth {
                    tracing::warn!("DEV_ONLY_NO_AUTH is set — authentication disabled");
                    None
                } else {
                    let (layer, _cache) =
                        web::auth::build_auth_layer(&cfg.database.url).await;
                    Some(layer)
                };

                // Rocket API on configured port (background task)
                let api_port = cfg.api.port;
                let api_pool = pool.clone();
                let api_rocket = api::build_rocket(api_pool, api_port)
                    .ignite()
                    .await
                    .expect("failed to ignite API rocket");
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

    #[cfg(not(feature = "server"))]
    {
        dioxus::launch(web::app::App);
    }
}
