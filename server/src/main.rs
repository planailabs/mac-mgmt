#[cfg(feature = "server")]
mod api;
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
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tracing_subscriber::fmt::init();

                let pool = db::connect().await;

                sqlx::migrate!()
                    .run(&pool)
                    .await
                    .expect("failed to run migrations");

                server_state::set_pool(pool.clone());

                // Rocket API on port 8080 (background task)
                let api_pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) = api::build_rocket(api_pool, 8080).launch().await {
                        tracing::error!("API server failed: {e}");
                    }
                });

                // Dioxus fullstack on port 3000
                dioxus::launch(web::app::App);
            });
    }

    #[cfg(not(feature = "server"))]
    {
        dioxus::launch(web::app::App);
    }
}
