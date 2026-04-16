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

use crate::orchestrator::{CellStatus, Orchestrator, StatusSnapshot};

#[derive(Debug, Serialize, Deserialize)]
pub struct Ack {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[rocket::get("/status")]
async fn api_status(orch: &State<Arc<Orchestrator>>) -> Json<StatusSnapshot> {
    Json(orch.inner().snapshot().await)
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

#[rocket::post("/gc")]
async fn api_gc(orch: &State<Arc<Orchestrator>>) -> Result<Json<Ack>, Status> {
    orch.inner().gc().await.map_err(|e| {
        tracing::error!("gc failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail: None }))
}

#[rocket::post("/chaos")]
async fn api_chaos(orch: &State<Arc<Orchestrator>>) -> Result<Json<Ack>, Status> {
    let detail = orch.inner().chaos_tick().await.map_err(|e| {
        tracing::error!("chaos failed: {e:#}");
        Status::InternalServerError
    })?;
    Ok(Json(Ack { ok: true, detail }))
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

// ── HTML dashboard ─────────────────────────────────────────────────────

#[rocket::get("/")]
async fn ui_index(orch: &State<Arc<Orchestrator>>) -> RawHtml<String> {
    let snap = orch.inner().snapshot().await;
    RawHtml(render_index(&snap))
}

fn render_index(snap: &StatusSnapshot) -> String {
    let mut rows = String::new();
    for c in &snap.cells {
        rows.push_str(&render_cell_row(c));
    }

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>mac-mgmt-runner</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{ font-family: -apple-system, system-ui, sans-serif; margin: 1.5rem; background: Canvas; color: CanvasText; }}
  h1 {{ font-size: 1.25rem; margin: 0 0 0.75rem; }}
  .bar {{ display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-bottom: 1rem; }}
  .bar span.summary {{ margin-left: auto; font-size: 0.9rem; color: GrayText; }}
  button {{ font: inherit; padding: 0.4rem 0.75rem; border: 1px solid #888; background: #f5f5f5; color: #222; border-radius: 4px; cursor: pointer; }}
  button:hover {{ background: #e5e5e5; }}
  button.danger {{ background: #fdd; border-color: #c00; color: #900; }}
  button.danger:hover {{ background: #fbb; }}
  table {{ border-collapse: collapse; width: 100%; font-size: 0.85rem; }}
  th, td {{ text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid #ddd; vertical-align: top; }}
  th {{ background: #f0f0f0; color: #222; font-weight: 600; position: sticky; top: 0; }}
  .mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.78rem; color: GrayText; }}
  .stage {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.78rem; padding: 0.1rem 0.4rem; border-radius: 4px; background: #eee; color: #333; }}
  .stage.running {{ background: #cfe9c9; color: #195a18; }}
  .stage.launching {{ background: #fde9a0; color: #6b4a00; }}
  .stage.pending, .stage.cluster_created, .stage.config_pushed {{ background: #dde; color: #334; }}
  .healthy-yes {{ color: #197a17; font-weight: 600; }}
  .healthy-no {{ color: #a01010; font-weight: 600; }}
  .healthy-unknown {{ color: #666; }}
  ul.instances {{ margin: 0; padding-left: 1rem; }}
  ul.instances li {{ line-height: 1.3; }}
  @media (prefers-color-scheme: dark) {{
    body {{ background: #1a1a1a; }}
    button {{ background: #2a2a2a; border-color: #555; color: #eee; }}
    button:hover {{ background: #3a3a3a; }}
    button.danger {{ background: #4a1515; color: #fdd; border-color: #c00; }}
    button.danger:hover {{ background: #5a1a1a; }}
    th {{ background: #2a2a2a; color: #eee; }}
    th, td {{ border-bottom-color: #333; }}
    .stage {{ background: #333; color: #ddd; }}
    .stage.running {{ background: #1c4a1c; color: #d7f2d0; }}
    .stage.launching {{ background: #553d04; color: #ffe9a0; }}
    .stage.pending, .stage.cluster_created, .stage.config_pushed {{ background: #333a55; color: #ccd; }}
  }}
</style>
<script>
async function runAction(path, confirmMsg) {{
  if (confirmMsg && !confirm(confirmMsg)) return;
  const btn = event.currentTarget;
  btn.disabled = true;
  btn.dataset.orig = btn.textContent;
  btn.textContent = "…";
  try {{
    const r = await fetch(path, {{ method: "POST" }});
    const body = await r.json().catch(() => ({{}}));
    if (!r.ok || body.ok === false) {{
      alert("Failed: " + r.status + " " + (body.detail || ""));
    }}
  }} catch (e) {{
    alert("Request failed: " + e);
  }}
  setTimeout(() => window.location.reload(), 500);
}}
// Poll /status every 5s and soft-refresh the table without a full reload.
async function refresh() {{
  try {{
    const r = await fetch("/status", {{ headers: {{ "Accept": "application/json" }} }});
    if (!r.ok) return;
    const snap = await r.json();
    document.querySelector(".summary").textContent =
      snap.total_cells + " cells · " + snap.running + " running";
    // Simple approach: reload the page so server-rendered markup stays
    // authoritative. Avoids rewriting the HTML from JS.
    window.location.reload();
  }} catch (_) {{}}
}}
setTimeout(refresh, 5000);
</script>
</head>
<body>
  <h1>mac-mgmt-runner</h1>
  <div class="bar">
    <button onclick="runAction('/provision')">Provision</button>
    <button onclick="runAction('/reprovision')">Reprovision random</button>
    <button onclick="runAction('/gc')">GC</button>
    <button onclick="runAction('/chaos')">Chaos tick</button>
    <button class="danger" onclick="runAction('/redeploy', 'Redeploy will destroy every matching cluster and rebuild. Continue?')">Redeploy</button>
    <button class="danger" onclick="runAction('/teardown', 'Teardown will destroy every provisioned cell. Continue?')">Teardown</button>
    <span class="summary">{total} cells · {running} running</span>
  </div>
  <table>
    <thead>
      <tr>
        <th>Cell</th>
        <th>Stage</th>
        <th>Nodes</th>
        <th>Cluster</th>
        <th>Instances</th>
        <th>Health</th>
        <th>Detail</th>
        <th>Launching since</th>
        <th>Fails</th>
      </tr>
    </thead>
    <tbody>
{rows}
    </tbody>
  </table>
</body>
</html>
"#,
        total = snap.total_cells,
        running = snap.running,
        rows = rows,
    )
}

fn render_cell_row(c: &CellStatus) -> String {
    let cluster = c
        .cluster_id
        .map(|id| format!("<span class=\"mono\">{id}</span>"))
        .unwrap_or_else(|| "—".to_string());
    let instances = if c.instances.is_empty() {
        "—".to_string()
    } else {
        let items: String = c
            .instances
            .iter()
            .map(|i| {
                let short: String = i.instance_id.chars().take(12).collect();
                format!(
                    "<li><span class=\"mono\">{}</span><br><span class=\"mono\" title=\"{}\">{}</span></li>",
                    html_escape(&i.instance_name),
                    html_escape(&i.instance_id),
                    html_escape(&short),
                )
            })
            .collect();
        format!("<ul class=\"instances\">{items}</ul>")
    };
    let health = match c.healthy {
        Some(true) => "<span class=\"healthy-yes\">✔ healthy</span>",
        Some(false) => "<span class=\"healthy-no\">✗ unhealthy</span>",
        None => "<span class=\"healthy-unknown\">—</span>",
    };
    let detail = c
        .detail
        .as_deref()
        .map(html_escape)
        .unwrap_or_default();
    let launching_since = c
        .launching_since
        .map(|t| t.format("%Y-%m-%d %H:%M:%SZ").to_string())
        .unwrap_or_else(|| "—".to_string());
    let stage_class = format!("stage {}", c.stage);
    format!(
        r#"      <tr>
        <td><strong>{key}</strong></td>
        <td><span class="{stage_class}">{stage}</span></td>
        <td>{nodes}</td>
        <td>{cluster}</td>
        <td>{instances}</td>
        <td>{health}</td>
        <td>{detail}</td>
        <td class="mono">{launching_since}</td>
        <td>{fails}</td>
      </tr>
"#,
        key = html_escape(&c.key),
        stage_class = stage_class,
        stage = html_escape(&c.stage),
        nodes = c.node_count,
        cluster = cluster,
        instances = instances,
        health = health,
        detail = detail,
        launching_since = launching_since,
        fails = c.deploy_failures,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
        let r = self.http.post(format!("{}/chaos", self.base)).send().await?;
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
