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

use crate::orchestrator::{Orchestrator, StatusSnapshot, status_snapshot};

#[derive(Debug, Serialize, Deserialize)]
pub struct Ack {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[rocket::get("/status")]
async fn api_status(orch: &State<Arc<Orchestrator>>) -> Json<StatusSnapshot> {
    Json(status_snapshot(orch.inner()).await)
}

#[rocket::post("/provision")]
async fn api_provision(orch: &State<Arc<Orchestrator>>) -> Result<Json<Ack>, Status> {
    orch.inner().reconcile().await.map_err(|e| {
        tracing::error!("provision failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail: None }))
}

#[rocket::post("/teardown")]
async fn api_teardown(orch: &State<Arc<Orchestrator>>) -> Result<Json<Ack>, Status> {
    orch.inner().teardown().await.map_err(|e| {
        tracing::error!("teardown failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail: None }))
}

#[rocket::post("/redeploy")]
async fn api_redeploy(orch: &State<Arc<Orchestrator>>) -> Result<Json<Ack>, Status> {
    orch.inner().redeploy().await.map_err(|e| {
        tracing::error!("redeploy failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail: None }))
}

#[rocket::post("/reprovision")]
async fn api_reprovision_random(
    orch: &State<Arc<Orchestrator>>,
) -> Result<Json<Ack>, Status> {
    let k = orch.inner().reprovision_random().await.map_err(|e| {
        tracing::error!("reprovision_random: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail: k }))
}

#[rocket::post("/reprovision/<key>")]
async fn api_reprovision_key(
    orch: &State<Arc<Orchestrator>>,
    key: &str,
) -> Result<Json<Ack>, Status> {
    orch.inner().reprovision_cell(key).await.map_err(|e| {
        tracing::error!("reprovision {key}: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail: Some(key.to_string()) }))
}

#[rocket::post("/shutdown")]
async fn api_shutdown(shutdown: Shutdown) -> Json<Ack> {
    shutdown.notify();
    Json(Ack { ok: true, detail: None })
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
                api_status,
                api_provision,
                api_teardown,
                api_redeploy,
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
        Self {
            base: format!("http://{host}:{port}"),
            http: reqwest::Client::new(),
        }
    }

    pub async fn status(&self) -> Result<StatusSnapshot> {
        let r = self.http.get(format!("{}/status", self.base)).send().await?;
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
