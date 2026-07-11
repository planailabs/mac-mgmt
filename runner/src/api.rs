//! Runner HTTP API (served by `mac-mgmt-runner daemon`) and a small HTTP client
//! used by the CLI subcommands.
//!
//! The daemon binds to a local address (default 127.0.0.1:9400) and exposes:
//!   GET  /status                      — fleet snapshot
//!   POST /provision                   — ensure-all (full reconcile)
//!   POST /teardown                    — destroy everything
//!   POST /reprovision                 — pick a random cell and reprovision
//!   POST /reprovision/<key>           — reprovision a specific cell

use std::sync::Arc;

use anyhow::Result;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{Shutdown, State};
use serde::{Deserialize, Serialize};

use rocket::response::content::RawHtml;

use crate::orchestrator::{Orchestrator, StatusSnapshot};
use crate::ui;

#[derive(Debug, Serialize, Deserialize)]
pub struct Ack {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The bearer token required for mutating endpoints, resolved from config or the
/// RUNNER_API_TOKEN env var. `None` means no token is configured.
fn expected_api_token(orch: &Orchestrator) -> Option<String> {
    orch.config
        .api
        .token
        .clone()
        .or_else(|| std::env::var("RUNNER_API_TOKEN").ok())
        .filter(|t| !t.is_empty())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Request guard enforcing the API bearer token on state-changing endpoints.
/// When no token is configured the guard passes (loopback-only default); when
/// one is configured a matching `Authorization: Bearer <token>` is required.
struct ApiAuth;

#[rocket::async_trait]
impl<'r> rocket::request::FromRequest<'r> for ApiAuth {
    type Error = ();
    async fn from_request(
        req: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let Some(orch) = req.rocket().state::<Arc<Orchestrator>>() else {
            return rocket::request::Outcome::Error((Status::InternalServerError, ()));
        };
        match expected_api_token(orch) {
            None => rocket::request::Outcome::Success(ApiAuth),
            Some(expected) => {
                let provided = req
                    .headers()
                    .get_one("authorization")
                    .and_then(|v| v.strip_prefix("Bearer "));
                match provided {
                    Some(p) if constant_time_eq(p, &expected) => {
                        rocket::request::Outcome::Success(ApiAuth)
                    }
                    _ => rocket::request::Outcome::Error((Status::Unauthorized, ())),
                }
            }
        }
    }
}

#[rocket::get("/status")]
async fn api_status(orch: &State<Arc<Orchestrator>>) -> Json<StatusSnapshot> {
    Json(orch.inner().snapshot().await)
}

#[rocket::post("/provision")]
async fn api_provision(
    _auth: ApiAuth,
    orch: &State<Arc<Orchestrator>>,
) -> Result<Json<Ack>, Status> {
    // Explicit operator action — resume from a prior teardown pause.
    if let Err(e) = orch.inner().resume().await {
        tracing::error!("resume before provision: {e:#}");
        return Err(Status::InternalServerError);
    }
    orch.inner().reconcile().await.map_err(|e| {
        tracing::error!("provision failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack {
        ok: true,
        detail: None,
    }))
}

#[rocket::post("/teardown")]
async fn api_teardown(
    _auth: ApiAuth,
    orch: &State<Arc<Orchestrator>>,
) -> Result<Json<Ack>, Status> {
    orch.inner().teardown().await.map_err(|e| {
        tracing::error!("teardown failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack {
        ok: true,
        detail: None,
    }))
}

#[rocket::post("/redeploy")]
async fn api_redeploy(
    _auth: ApiAuth,
    orch: &State<Arc<Orchestrator>>,
) -> Result<Json<Ack>, Status> {
    orch.inner().redeploy().await.map_err(|e| {
        tracing::error!("redeploy failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack {
        ok: true,
        detail: None,
    }))
}

#[rocket::post("/gc")]
async fn api_gc(_auth: ApiAuth, orch: &State<Arc<Orchestrator>>) -> Result<Json<Ack>, Status> {
    orch.inner().gc().await.map_err(|e| {
        tracing::error!("gc failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack {
        ok: true,
        detail: None,
    }))
}

#[rocket::post("/chaos")]
async fn api_chaos(_auth: ApiAuth, orch: &State<Arc<Orchestrator>>) -> Result<Json<Ack>, Status> {
    let detail = orch.inner().chaos_tick().await.map_err(|e| {
        tracing::error!("chaos failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail }))
}

#[rocket::post("/reprovision")]
async fn api_reprovision_random(
    _auth: ApiAuth,
    orch: &State<Arc<Orchestrator>>,
) -> Result<Json<Ack>, Status> {
    let k = orch.inner().reprovision_random().await.map_err(|e| {
        tracing::error!("reprovision_random: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack {
        ok: true,
        detail: k,
    }))
}

#[rocket::post("/reprovision/<key>")]
async fn api_reprovision_key(
    _auth: ApiAuth,
    orch: &State<Arc<Orchestrator>>,
    key: &str,
) -> Result<Json<Ack>, Status> {
    orch.inner().reprovision_cell(key).await.map_err(|e| {
        tracing::error!("reprovision {key}: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack {
        ok: true,
        detail: Some(key.to_string()),
    }))
}

#[rocket::post("/shutdown")]
async fn api_shutdown(_auth: ApiAuth, shutdown: Shutdown) -> Json<Ack> {
    shutdown.notify();
    Json(Ack {
        ok: true,
        detail: None,
    })
}

// ── HTML dashboard ─────────────────────────────────────────────────────

#[rocket::get("/")]
async fn ui_index(orch: &State<Arc<Orchestrator>>) -> RawHtml<String> {
    let snap = orch.inner().snapshot().await;
    RawHtml(ui::render_index(&snap))
}

pub async fn serve(orch: Arc<Orchestrator>) -> Result<()> {
    let bind = orch.config.api.bind.clone();
    let port = orch.config.api.port;
    let figment = rocket::Config::figment()
        .merge(("address", bind))
        .merge(("port", port))
        .merge(("log_level", "off"));
    rocket::custom(figment)
        .manage(orch)
        .mount(
            "/",
            rocket::routes![
                ui_index,
                api_status,
                api_provision,
                api_teardown,
                api_redeploy,
                api_gc,
                api_chaos,
                api_reprovision_random,
                api_reprovision_key,
                api_shutdown,
            ],
        )
        .launch()
        .await?;
    Ok(())
}

// ── CLI client ────────────────────────────────────────────────────────

pub struct Cli {
    base: String,
    http: reqwest::Client,
}

impl Cli {
    pub fn new(bind: &str, port: u16) -> Self {
        let host = if bind == "0.0.0.0" || bind.is_empty() {
            "127.0.0.1"
        } else {
            bind
        };
        // Send the API token (if configured) on every request so mutating
        // endpoints authorize. Matches the server-side RUNNER_API_TOKEN guard.
        let mut builder = reqwest::Client::builder();
        if let Ok(token) = std::env::var("RUNNER_API_TOKEN") {
            if !token.is_empty() {
                let mut headers = reqwest::header::HeaderMap::new();
                if let Ok(mut v) =
                    reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                {
                    v.set_sensitive(true);
                    headers.insert(reqwest::header::AUTHORIZATION, v);
                    builder = builder.default_headers(headers);
                }
            }
        }
        Self {
            base: format!("http://{host}:{port}"),
            http: builder.build().unwrap_or_default(),
        }
    }

    pub async fn status(&self) -> Result<StatusSnapshot> {
        let r = self
            .http
            .get(format!("{}/status", self.base))
            .send()
            .await?;
        r.error_for_status_ref()?;
        Ok(r.json().await?)
    }

    pub async fn provision(&self) -> Result<Ack> {
        let r = self
            .http
            .post(format!("{}/provision", self.base))
            .send()
            .await?;
        r.error_for_status_ref()?;
        Ok(r.json().await?)
    }

    pub async fn teardown(&self) -> Result<Ack> {
        let r = self
            .http
            .post(format!("{}/teardown", self.base))
            .send()
            .await?;
        r.error_for_status_ref()?;
        Ok(r.json().await?)
    }

    pub async fn redeploy(&self) -> Result<Ack> {
        let r = self
            .http
            .post(format!("{}/redeploy", self.base))
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .await?;
        r.error_for_status_ref()?;
        Ok(r.json().await?)
    }

    pub async fn gc(&self) -> Result<Ack> {
        let r = self
            .http
            .post(format!("{}/gc", self.base))
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await?;
        r.error_for_status_ref()?;
        Ok(r.json().await?)
    }

    pub async fn chaos(&self) -> Result<Ack> {
        let r = self
            .http
            .post(format!("{}/chaos", self.base))
            .send()
            .await?;
        r.error_for_status_ref()?;
        Ok(r.json().await?)
    }

    pub async fn reprovision(&self, key: Option<&str>) -> Result<Ack> {
        let url = match key {
            Some(k) => format!("{}/reprovision/{k}", self.base),
            None => format!("{}/reprovision", self.base),
        };
        let r = self.http.post(url).send().await?;
        r.error_for_status_ref()?;
        Ok(r.json().await?)
    }
}

#[cfg(test)]
mod auth_tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches_semantics() {
        assert!(constant_time_eq("s3cret", "s3cret"));
        assert!(!constant_time_eq("s3cret", "s3crey"));
        assert!(!constant_time_eq("s3cret", "s3cre"));
        assert!(!constant_time_eq("", "x"));
        assert!(constant_time_eq("", ""));
    }
}
