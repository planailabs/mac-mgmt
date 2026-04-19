#[cfg(feature = "webui")]
mod anthropic;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod api;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod config;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod db;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod mcp_schema;
#[cfg(feature = "webui")]
mod models;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod rollout_health;
#[cfg(feature = "webui")]
mod web;
#[cfg(any(feature = "server", feature = "server-api-only"))]
mod xzar;

#[cfg(all(feature = "server", feature = "webui"))]
mod server_state {
    use crate::api::push::PushChannels;
    use mac_mgmt_healer::HealerState;
    use sqlx::PgPool;
    use std::sync::OnceLock;

    static POOL: OnceLock<PgPool> = OnceLock::new();
    static PUSH: OnceLock<PushChannels> = OnceLock::new();
    static HEALER: OnceLock<HealerState> = OnceLock::new();

    pub fn set_pool(pool: PgPool) {
        POOL.set(pool).expect("pool already initialized");
    }

    pub fn set_push_channels(channels: PushChannels) {
        PUSH.set(channels)
            .expect("push channels already initialized");
    }

    pub fn set_healer_state(state: HealerState) {
        let _ = HEALER.set(state);
    }

    pub fn server_pool() -> Result<PgPool, dioxus::prelude::ServerFnError> {
        POOL.get()
            .cloned()
            .ok_or_else(|| dioxus::prelude::ServerFnError::new("database pool not initialized"))
    }

    pub fn push_channels() -> Result<PushChannels, dioxus::prelude::ServerFnError> {
        PUSH.get()
            .cloned()
            .ok_or_else(|| dioxus::prelude::ServerFnError::new("push channels not initialized"))
    }

    pub fn healer_state() -> Option<HealerState> {
        HEALER.get().cloned()
    }
}

#[cfg(all(feature = "server", feature = "webui"))]
pub fn server_pool() -> Result<sqlx::PgPool, dioxus::prelude::ServerFnError> {
    server_state::server_pool()
}

#[cfg(all(feature = "server", feature = "webui"))]
pub fn push_channels() -> Result<crate::api::push::PushChannels, dioxus::prelude::ServerFnError> {
    server_state::push_channels()
}

#[cfg(any(feature = "server", feature = "server-api-only"))]
async fn init_server() -> (
    sqlx::PgPool,
    rocket::Rocket<rocket::Ignite>,
    mac_mgmt_healer::HealerState,
) {
    let cfg = config::load();
    let pool = db::connect(&cfg.database.url).await;

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run migrations");

    let push_channels = api::push::new_push_channels();

    #[cfg(feature = "webui")]
    server_state::set_pool(pool.clone());
    #[cfg(feature = "webui")]
    server_state::set_push_channels(push_channels.clone());

    // Background task: delete daemon heartbeats offline for 30+ days
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            loop {
                match sqlx::query(
                    "DELETE FROM daemon_heartbeats WHERE reported_at < now() - interval '30 days'",
                )
                .execute(&pool)
                .await
                {
                    Ok(result) => {
                        let n = result.rows_affected();
                        if n > 0 {
                            tracing::info!("cleaned up {n} stale daemon heartbeats (offline >30d)");
                        }
                    }
                    Err(e) => tracing::error!("heartbeat cleanup failed: {e}"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });
    }

    // Background task: auto-pause rollout stages that fail their health gate.
    {
        let pool = pool.clone();
        tokio::spawn(rollout_health::run_auto_pause_loop(
            pool,
            std::time::Duration::from_secs(60),
        ));
    }

    // Initialize healer state
    let healer_connector = mac_mgmt_healer::ConnectorConfig {
        ollama_url: cfg.healer.ollama_url.clone(),
        ollama_model: cfg.healer.ollama_model.clone(),
        anthropic_api_key: cfg.anthropic.as_ref().map(|a| a.api_key.clone()),
        anthropic_model: cfg.healer.anthropic_model.clone(),
        token_budget: cfg.healer.token_budget,
    };
    let healer_state = mac_mgmt_healer::HealerState::new(pool.clone(), healer_connector);
    #[cfg(feature = "webui")]
    server_state::set_healer_state(healer_state.clone());

    // Resume healer sessions interrupted by a previous shutdown
    {
        let healer = healer_state.clone();
        tokio::spawn(async move {
            match healer.resume_interrupted().await {
                Ok(n) if n > 0 => tracing::info!("resumed {n} interrupted healer sessions"),
                Ok(_) => {}
                Err(e) => tracing::error!("failed to resume healer sessions: {e}"),
            }
        });
    }

    let api_rocket = api::build_rocket(
        pool.clone(),
        cfg.api.port,
        push_channels,
        healer_state.clone(),
    )
    .ignite()
    .await
    .expect("failed to ignite API rocket");

    (pool, api_rocket, healer_state)
}

/// Install a tracing subscriber that mirrors events to stdout and, when
/// Sentry is initialised, forwards ERROR/WARN events as Sentry breadcrumbs
/// and errors. Safe to call from either server-api-only or the full webui
/// mode — the webui path otherwise relies on whatever dioxus sets up.
#[cfg(any(feature = "server", feature = "server-api-only"))]
fn init_tracing() {
    mac_mgmt_common::tracing_init::init_tracing_with_sentry("info");
}

#[cfg(any(feature = "server", feature = "server-api-only"))]
fn init_sentry() -> Option<sentry::ClientInitGuard> {
    let cfg = config::load();
    mac_mgmt_common::sentry_ext::init_sentry(&cfg.sentry)
}

fn main() {
    // Sentry must be initialised on the main thread before any runtime
    // spins up so the panic handler is installed globally. The guard must
    // live for the lifetime of the process. We set up tracing first so
    // sentry's tracing layer can forward events.
    #[cfg(any(feature = "server", feature = "server-api-only"))]
    let _sentry_guard = init_sentry();
    #[cfg(all(feature = "server", feature = "webui"))]
    init_tracing();

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
                    // Drain healer sessions before stopping
                    if let Some(healer) = crate::server_state::healer_state() {
                        let drained = healer
                            .graceful_shutdown(std::time::Duration::from_secs(30))
                            .await;
                        if drained > 0 {
                            tracing::info!("drained {drained} healer sessions");
                        }
                    }
                    if let Some(handle) = ROCKET_SHUTDOWN.get() {
                        handle.clone().notify();
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                    std::process::exit(0);
                });
        });

        dioxus::serve(move || async move {
            let dev_no_auth = std::env::var("DEV_ONLY_NO_AUTH").as_deref() == Ok("1");
            let auth_layer = if let Some(layer) = INIT.get() {
                layer.clone()
            } else {
                let (_pool, api_rocket, _healer_state) = init_server().await;
                let cfg = config::load();

                let auth_layer = if dev_no_auth {
                    tracing::warn!("DEV_ONLY_NO_AUTH=1 — OIDC disabled, using dev admin user");
                    // Insert dev user once at startup.
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
                } else if cfg.oidc.is_some() {
                    let (layer, _cache) = web::auth::build_auth_layer(&cfg.database.url).await;
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

            let mut router =
                axum::Router::new().serve_dioxus_application(ServeConfig::new(), web::app::App);

            if let Some(auth_layer) = auth_layer {
                router = router
                    .layer(axum::middleware::from_fn(web::auth::require_auth))
                    .layer(auth_layer);
            } else if dev_no_auth {
                // DEV mode: add require_auth middleware (for dev user injection) without OIDC layer
                router = router.layer(axum::middleware::from_fn(web::auth::require_auth));
            }

            Ok(router)
        });
    }

    // API-only mode: no web UI, just Rocket
    #[cfg(feature = "server-api-only")]
    {
        init_tracing();
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async {
            let (_pool, api_rocket, healer_state) = init_server().await;

            let shutdown = api_rocket.shutdown();
            tokio::spawn(async move {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler")
                    .recv()
                    .await;
                tracing::info!("received SIGTERM, draining healer sessions...");
                let drained = healer_state
                    .graceful_shutdown(std::time::Duration::from_secs(30))
                    .await;
                if drained > 0 {
                    tracing::info!("drained {drained} healer sessions");
                }
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
