#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
mod crypto;
#[cfg(feature = "server")]
mod db;
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

    #[cfg(feature = "webui")]
    server_state::set_pool(pool.clone());

    pool
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
        use std::sync::OnceLock;

        static INIT: OnceLock<Option<Vec<axum_oidc_client::auth::AuthLayer>>> = OnceLock::new();

        if std::env::var("PORT").is_err() {
            let cfg = config::load();
            unsafe { std::env::set_var("PORT", cfg.web.port.to_string()) };
        }

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
                } else if cfg.auth.is_some() {
                    let (layers, _cache) = web::auth::build_auth_layers(&cfg.database.url).await;
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

            if let Some(auth_layers) = auth_layers {
                router = router
                    .route("/auth/login", axum::routing::get(web::auth::login_page))
                    .route("/auth/logout", axum::routing::get(web::auth::logout_handler))
                    .layer(axum::middleware::from_fn(web::auth::require_auth));
                for layer in auth_layers {
                    router = router.layer(layer);
                }
            } else if dev_no_auth {
                router = router
                    .layer(axum::middleware::from_fn(web::auth::require_auth));
            }

            Ok(router)
        });
    }

    #[cfg(all(not(feature = "server"), feature = "webui"))]
    {
        dioxus::launch(web::app::App);
    }
}
