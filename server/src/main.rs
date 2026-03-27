#[cfg(feature = "server")]
mod api;
#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
mod db;
mod models;
mod web;

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

        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tracing_subscriber::fmt::init();

                let cfg = config::load();
                let pool = db::connect(&cfg.database.url).await;

                sqlx::migrate!()
                    .run(&pool)
                    .await
                    .expect("failed to run migrations");

                server_state::set_pool(pool.clone());

                // Build OIDC auth layer and session cache
                let (auth_layer, _cache) =
                    web::auth::build_auth_layer(&cfg.database.url).await;

                // Rocket API on configured port (background task)
                let api_port = cfg.api.port;
                let api_pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) = api::build_rocket(api_pool, api_port).launch().await {
                        tracing::error!("API server failed: {e}");
                    }
                });

                // Dioxus fullstack with OIDC auth on configured port
                dioxus::serve(|| {
                    let auth_layer = auth_layer.clone();
                    async move {
                        let router = axum::Router::new()
                            .serve_dioxus_application(ServeConfig::new(), web::app::App)
                            .layer(axum::middleware::from_fn(web::auth::require_auth))
                            .layer(auth_layer);

                        Ok(router)
                    }
                });
            });
    }

    #[cfg(not(feature = "server"))]
    {
        dioxus::launch(web::app::App);
    }
}
