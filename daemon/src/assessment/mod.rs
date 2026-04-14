#![allow(dead_code)] // scaffolding — items become live as later steps (5–9) land.

//! System assessment module.
//!
//! Three tiers of collection, each on its own cadence:
//!
//! * **Sample** — cheap dynamic numbers (CPU/mem/disk/net) piggybacked on every heartbeat.
//! * **Inventory** — static facts + security posture, refreshed every ~6h, on startup,
//!   and on `PushCommand::RequestAssessment`. Delivered via `POST /api/assessment`.
//! * **Probe** — functional end-to-end round-trips through managed services (full-prompt
//!   canaries for LLM backends, etc.). Every ~15 min (jittered). Delivered via
//!   `POST /api/assessment/probe`.

pub mod inventory;
pub mod probes;
pub mod sample;
pub mod security;

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use tokio::sync::RwLock;

use mac_mgmt_common::{Assessment, DaemonConfig, DynamicSample, ProbeReport, ServiceExtState};

use crate::metrics::Metrics;

/// Default cadence for deep probes. Jittered ±2min.
pub const DEFAULT_PROBE_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// Default cadence for full inventory refresh.
pub const DEFAULT_INVENTORY_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Orchestrates assessment collection. Holds the latest rolled-up summaries so the
/// heartbeat sender can piggyback without async work.
pub struct Assessor {
    latest_sample: Arc<RwLock<Option<DynamicSample>>>,
    latest_probes: Arc<RwLock<Vec<ServiceExtState>>>,
    /// Snapshot of the most recent DaemonConfig so probes can derive per-service
    /// endpoints (ollama host/port, openclaw gateway, etc.) without reaching
    /// back into the daemon event loop.
    config: Arc<RwLock<Option<DaemonConfig>>>,
    /// Prometheus metrics surface. Optional so tests can skip wiring.
    metrics: Arc<RwLock<Option<Arc<Metrics>>>>,
}

impl Assessor {
    pub fn new() -> Self {
        Self {
            latest_sample: Arc::new(RwLock::new(None)),
            latest_probes: Arc::new(RwLock::new(Vec::new())),
            config: Arc::new(RwLock::new(None)),
            metrics: Arc::new(RwLock::new(None)),
        }
    }

    /// Attach the shared metrics instance so sample/inventory/probe updates
    /// land on Prometheus gauges.
    pub async fn attach_metrics(&self, metrics: Arc<Metrics>) {
        *self.metrics.write().await = Some(metrics);
    }

    /// Replace the stored config snapshot. Call on startup and on every config
    /// reload so probes see fresh endpoints.
    pub async fn update_config(&self, cfg: DaemonConfig) {
        *self.config.write().await = Some(cfg);
    }

    /// Snapshot the latest dynamic sample for piggybacking into a heartbeat.
    pub fn latest_sample_snapshot(&self) -> Option<DynamicSample> {
        self.latest_sample.try_read().ok().and_then(|g| g.clone())
    }

    /// Snapshot the latest probe summaries.
    pub fn latest_probes_snapshot(&self) -> Vec<ServiceExtState> {
        self.latest_probes
            .try_read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Collect a fresh dynamic sample and update the snapshot. Cheap (<50ms).
    pub async fn refresh_sample(&self) {
        match sample::collect().await {
            Ok(s) => {
                if let Some(m) = self.metrics.read().await.clone() {
                    m.assessment.update_sample(&s);
                }
                let mut w = self.latest_sample.write().await;
                *w = Some(s);
            }
            Err(e) => {
                tracing::debug!("sample collection failed: {e}");
            }
        }
    }

    /// Build and send a full inventory+security assessment. Called on startup, every
    /// inventory interval, and on `RequestAssessment` push.
    pub async fn send_inventory(
        &self,
        server_url: &str,
        server_token: &str,
        instance_id: &str,
        host_key: &russh::keys::PrivateKey,
    ) {
        let inventory = match inventory::collect().await {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("inventory collection failed: {e}");
                return;
            }
        };
        let security = match security::collect().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("security collection failed: {e}");
                return;
            }
        };
        if let Some(m) = self.metrics.read().await.clone() {
            m.assessment.update_inventory(&inventory);
            m.assessment.update_security(&security);
        }

        let collected_at = chrono::Utc::now().timestamp();
        let body = match build_signed_assessment(
            instance_id,
            collected_at,
            inventory,
            security,
            host_key,
        ) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("failed to sign assessment: {e}");
                return;
            }
        };

        post_assessment(server_url, server_token, body).await;
    }

    /// Run every configured probe and send each result. Expensive (full LLM inference).
    pub async fn run_probes(
        &self,
        server_url: &str,
        server_token: &str,
        instance_id: &str,
        host_key: &russh::keys::PrivateKey,
    ) {
        let cfg = match self.config.read().await.clone() {
            Some(c) => c,
            None => {
                tracing::debug!("assessment: no config snapshot, skipping probes");
                return;
            }
        };
        let ctx = probes::ProbeCtx::default();
        let probes = probes::registry(&cfg);
        let mut summaries: Vec<ServiceExtState> = Vec::with_capacity(probes.len());

        let metrics = self.metrics.read().await.clone();
        for probe in probes {
            let name = probe.name();
            let kind = probe.kind();
            let result = probe.run(&ctx).await;
            let collected_at = chrono::Utc::now().timestamp();

            if let Some(ref m) = metrics {
                m.assessment.update_probe(name, kind, &result);
            }

            summaries.push(ServiceExtState {
                name: name.to_string(),
                healthy: result.ok,
                last_probe_ok: Some(result.ok),
                last_probe_at: Some(collected_at),
                last_probe_duration_ms: Some(result.duration_ms),
                last_probe_kind: Some(kind.as_str().to_string()),
            });

            let body = match build_signed_probe(
                instance_id,
                collected_at,
                name,
                kind,
                &result,
                host_key,
            ) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("failed to sign probe {name}: {e}");
                    continue;
                }
            };
            post_probe(server_url, server_token, body).await;
        }

        *self.latest_probes.write().await = summaries;
    }

    /// Called on `PushCommand::RequestAssessment` — runs both inventory and probes
    /// immediately.
    pub async fn request(
        &self,
        server_url: &str,
        server_token: &str,
        instance_id: &str,
        host_key: &russh::keys::PrivateKey,
    ) {
        tracing::info!("assessment: on-demand snapshot requested");
        self.refresh_sample().await;
        self.send_inventory(server_url, server_token, instance_id, host_key).await;
        self.run_probes(server_url, server_token, instance_id, host_key).await;
    }
}

impl Default for Assessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Jitter a duration by up to ±`jitter_secs` seconds.
#[allow(dead_code)]
pub fn jittered(base: Duration, jitter_secs: u64) -> Duration {
    use rand::Rng;
    let jitter = rand::thread_rng().gen_range(0..=(jitter_secs * 2)) as i64 - jitter_secs as i64;
    let secs = (base.as_secs() as i64 + jitter).max(1) as u64;
    Duration::from_secs(secs)
}

fn build_signed_assessment(
    instance_id: &str,
    collected_at: i64,
    inventory: mac_mgmt_common::Inventory,
    security: mac_mgmt_common::SecurityPosture,
    host_key: &russh::keys::PrivateKey,
) -> anyhow::Result<Assessment> {
    use russh::keys::PublicKeyBase64;
    use russh::keys::signature::Signer;

    let message = format!("{instance_id}:{collected_at}");
    let sig = host_key.try_sign(message.as_bytes())?;
    let public_key = host_key.public_key_base64();
    let signature = base64::engine::general_purpose::STANDARD.encode(sig.as_bytes());

    Ok(Assessment {
        instance_id: instance_id.to_string(),
        collected_at,
        inventory,
        security,
        public_key,
        signature,
    })
}

async fn post_assessment(server_url: &str, server_token: &str, body: Assessment) {
    let url = format!("{server_url}/api/assessment");
    let client = reqwest::Client::new();
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        client
            .post(&url)
            .bearer_auth(server_token)
            .json(&body)
            .send(),
    )
    .await;
    match result {
        Ok(Ok(resp)) if resp.status().is_success() => {
            tracing::debug!("assessment accepted");
        }
        Ok(Ok(resp)) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("assessment rejected: {status} — {text}");
        }
        Ok(Err(e)) => tracing::warn!("assessment send failed: {e}"),
        Err(_) => tracing::warn!("assessment send timed out"),
    }
}

fn build_signed_probe(
    instance_id: &str,
    collected_at: i64,
    service: &str,
    kind: probes::ProbeKind,
    r: &probes::ProbeResult,
    host_key: &russh::keys::PrivateKey,
) -> anyhow::Result<ProbeReport> {
    use russh::keys::PublicKeyBase64;
    use russh::keys::signature::Signer;

    let message = format!("{instance_id}:{collected_at}");
    let sig = host_key.try_sign(message.as_bytes())?;
    let public_key = host_key.public_key_base64();
    let signature = base64::engine::general_purpose::STANDARD.encode(sig.as_bytes());

    Ok(ProbeReport {
        instance_id: instance_id.to_string(),
        collected_at,
        service: service.to_string(),
        kind: kind.as_str().to_string(),
        ok: r.ok,
        duration_ms: r.duration_ms,
        tokens_in: r.tokens_in,
        tokens_out: r.tokens_out,
        first_token_ms: r.first_token_ms,
        model: r.model.clone(),
        canary_digest: r.canary_digest.clone(),
        error_class: r.error_class.clone(),
        error_detail: r.error_detail.clone(),
        public_key,
        signature,
    })
}

async fn post_probe(server_url: &str, server_token: &str, body: ProbeReport) {
    let url = format!("{server_url}/api/assessment/probe");
    let client = reqwest::Client::new();
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        client
            .post(&url)
            .bearer_auth(server_token)
            .json(&body)
            .send(),
    )
    .await;
    match result {
        Ok(Ok(resp)) if resp.status().is_success() => {
            tracing::debug!("probe accepted: {}", body.service);
        }
        Ok(Ok(resp)) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("probe rejected ({}): {status} — {text}", body.service);
        }
        Ok(Err(e)) => tracing::warn!("probe send failed ({}): {e}", body.service),
        Err(_) => tracing::warn!("probe send timed out ({})", body.service),
    }
}

