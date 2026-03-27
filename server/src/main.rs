mod api;
mod db;
mod models;
mod web;

use sqlx::PgPool;
use std::sync::OnceLock;

static POOL: OnceLock<PgPool> = OnceLock::new();

/// Access the database pool from server functions.
pub fn server_pool() -> Result<PgPool, dioxus::prelude::ServerFnError> {
    POOL.get()
        .cloned()
        .ok_or_else(|| dioxus::prelude::ServerFnError::new("database pool not initialized"))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = db::connect().await;

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run migrations");

    POOL.set(pool.clone()).expect("pool already initialized");

    // Rocket API on port 8080 (background task)
    let api_pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = api::build_rocket(api_pool, 8080).launch().await {
            tracing::error!("API server failed: {e}");
        }
    });

    // Dioxus fullstack on port 3000 (main thread)
    dioxus::launch(web::app::App);
}
