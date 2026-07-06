//! mmrcd core: resolve images from CI, spin up incus instances per run, tear
//! them down. Instances launch via `mac-mgmt-executor`'s IncusBackend (REST);
//! per-run project + network lifecycle uses the `incus` CLI (no REST binding in
//! the executor for those).

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use mac_mgmt_executor::{IncusBackend, LaunchSpec};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::api::*;
use crate::hostkey;

/// mmrcd config (`~/.config/mmrcd/config.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct MmrcdConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    pub token: String,
    #[serde(default = "default_backend")]
    pub incus_backend: String,
    #[serde(default)]
    pub incus_project_prefix: Option<String>,
    /// HTTPS incus endpoint (e.g. "https://[2a01:...]:8443"). Required when
    /// incus_backend = "https".
    #[serde(default)]
    pub incus_url: Option<String>,
    /// Client cert / key PEM paths for mTLS to the HTTPS incus endpoint.
    #[serde(default)]
    pub incus_client_cert: Option<String>,
    #[serde(default)]
    pub incus_client_key: Option<String>,
    /// Optional server CA PEM path (else the server cert is not verified).
    #[serde(default)]
    pub incus_server_ca: Option<String>,
    #[serde(default = "default_registry")]
    pub registry: String,
    #[serde(default = "default_gitlab_url")]
    pub gitlab_url: String,
    #[serde(default = "default_gitlab_project")]
    pub gitlab_project: String,
    #[serde(default)]
    pub gitlab_token: Option<String>,
    #[serde(default = "default_ref")]
    pub default_ref: String,
    #[serde(default = "default_run_dir")]
    pub run_dir: PathBuf,
    /// Optional explicit image tag override (skips CI resolution) for dev.
    #[serde(default)]
    pub pinned_tag: Option<String>,
    /// How long (seconds) to wait for a launched instance to reach "Running".
    /// OCI/docker images pull + boot systemd, so this defaults to 15 minutes.
    #[serde(default = "default_ready_timeout")]
    pub instance_ready_timeout_secs: u64,
    /// Interval (seconds) between garbage-collection sweeps of stale run
    /// projects. Default 5 minutes; a sweep also runs at startup.
    #[serde(default = "default_gc_interval")]
    pub gc_interval_secs: u64,
}

fn default_ready_timeout() -> u64 {
    900
}
fn default_gc_interval() -> u64 {
    300
}

fn default_listen() -> String {
    "127.0.0.1:7390".into()
}
fn default_backend() -> String {
    "unix".into()
}
fn default_registry() -> String {
    "registry.plan.ai/plan-ai/mac-mgmt".into()
}
fn default_gitlab_url() -> String {
    "https://git.plan.ai".into()
}
fn default_gitlab_project() -> String {
    "plan-ai/mac-mgmt".into()
}
fn default_ref() -> String {
    "trunk".into()
}
fn default_run_dir() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("mmrcd/runs")
}

impl MmrcdConfig {
    pub fn load(path: Option<&std::path::Path>) -> Result<Self> {
        let path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("mmrcd/config.toml"));
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading mmrcd config {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }
}

/// Resolve registry image tags for a git ref by querying the latest successful
/// GitLab pipeline.
pub async fn resolve_images(cfg: &MmrcdConfig, git_ref: &str) -> Result<ImageStatus> {
    let (short_sha, status) = if let Some(tag) = &cfg.pinned_tag {
        (tag.clone(), "pinned".to_string())
    } else {
        let (sha, status) = latest_pipeline_sha(cfg, git_ref).await?;
        (sha.chars().take(8).collect(), status)
    };
    let img = |kind: &str| format!("{}/test-mac-mgmt-{kind}:{short_sha}", cfg.registry);
    let server_image = img("server");
    let relay_image = img("relay");
    let daemon_image = img("daemon");
    Ok(ImageStatus {
        git_ref: git_ref.to_string(),
        short_sha,
        pipeline_status: status,
        server_image,
        relay_image,
        daemon_image,
    })
}

async fn latest_pipeline_sha(cfg: &MmrcdConfig, git_ref: &str) -> Result<(String, String)> {
    #[derive(Deserialize)]
    struct Pipeline {
        sha: String,
        status: String,
    }
    let project = urlencoding::encode(&cfg.gitlab_project);
    let url = format!(
        "{}/api/v4/projects/{project}/pipelines?ref={git_ref}&status=success&per_page=1",
        cfg.gitlab_url
    );
    let mut req = reqwest::Client::new().get(&url);
    if let Some(tok) = &cfg.gitlab_token {
        req = req.header("PRIVATE-TOKEN", tok);
    }
    let resp = req.send().await.context("querying GitLab pipelines")?;
    if !resp.status().is_success() {
        bail!("GitLab pipelines query returned {}", resp.status());
    }
    let pipelines: Vec<Pipeline> = resp.json().await.context("parsing pipelines")?;
    let p = pipelines
        .into_iter()
        .next()
        .context("no successful pipeline for ref")?;
    Ok((p.sha, p.status))
}

/// Persisted per-run state (post-mortem visibility, not resume).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunState {
    pub run_id: String,
    pub project: String,
    pub image_tag: String,
    pub servers: Vec<ServerEndpoint>,
    pub node_instances: Vec<String>,
    pub status: String,
}

pub struct Orchestrator {
    cfg: MmrcdConfig,
    /// Project names currently held by a live run in this process. Anything
    /// prefix-matching but NOT in this set is stale and GC-eligible.
    active: Mutex<std::collections::HashSet<String>>,
}

impl Orchestrator {
    pub fn new(cfg: MmrcdConfig) -> Self {
        Self {
            cfg,
            active: Mutex::new(std::collections::HashSet::new()),
        }
    }

    pub fn config(&self) -> &MmrcdConfig {
        &self.cfg
    }

    fn project_name(&self, run_id: &str) -> String {
        let prefix = self.cfg.incus_project_prefix.as_deref().unwrap_or("mmrc");
        format!("{prefix}-{run_id}")
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.cfg.run_dir.join(run_id)
    }

    /// Build an incus backend scoped to a specific project.
    fn backend(&self, project: &str) -> Result<Arc<dyn IncusBackend>> {
        match self.cfg.incus_backend.as_str() {
            "unix" => Ok(Arc::new(mac_mgmt_executor::UnixBackend::new(Some(
                project.to_string(),
            )))),
            "https" => {
                let url = self
                    .cfg
                    .incus_url
                    .as_deref()
                    .context("incus_url required for https backend")?;
                let cert = self
                    .cfg
                    .incus_client_cert
                    .as_deref()
                    .context("incus_client_cert required for https backend")?;
                let key = self
                    .cfg
                    .incus_client_key
                    .as_deref()
                    .context("incus_client_key required for https backend")?;
                Ok(Arc::new(
                    mac_mgmt_executor::HttpsBackend::new(
                        url,
                        cert,
                        key,
                        self.cfg.incus_server_ca.as_deref(),
                        Some(project.to_string()),
                    )
                    .context("initializing https incus backend")?,
                ))
            }
            other => bail!("unknown incus_backend {other:?}"),
        }
    }

    // ── Project lifecycle (via the backend's generic raw incus API) ─────

    /// Create an ephemeral run project. `features.profiles=false` so instances
    /// inherit the default project's profile (root disk + nic); shared networks.
    async fn create_project(&self, name: &str) -> Result<()> {
        // Project-management calls are not project-scoped; use a default-scoped
        // backend so the acting-project query points at an existing project.
        let mgmt = self.backend("default")?;
        let mut config = serde_json::Map::new();
        config.insert("features.profiles".into(), serde_json::json!("false"));
        config.insert("features.images".into(), serde_json::json!("false"));
        config.insert("features.networks.zones".into(), serde_json::json!("true"));
        config.insert("features.storage.buckets".into(), serde_json::json!("true"));
        config.insert("features.storage.volumes".into(), serde_json::json!("true"));
        mgmt.create_project(name, config).await
    }

    /// Force-delete every instance in `project`, then the project itself.
    async fn destroy_project(&self, project: &str) -> Result<()> {
        let backend = self.backend(project)?;
        // List + delete each instance (delete stops first if running).
        if let Ok(names) = backend.project_instance_names().await {
            for name in names {
                let _ = backend.delete(&name).await;
            }
        }
        let mgmt = self.backend("default")?;
        let _ = mgmt.delete_project(project).await;
        Ok(())
    }

    /// Create a fresh run: project + network, then launch servers and their
    /// relays from the CI-resolved images. Returns the run info.
    pub async fn create_run(&self, run_id: &str, req: &CreateRunRequest) -> Result<RunInfo> {
        let project = self.project_name(run_id);
        // Mark active up front so a concurrent GC never reaps a run mid-build.
        self.active.lock().await.insert(project.clone());
        let result = self.create_run_inner(run_id, req, project.clone()).await;
        if result.is_err() {
            // Partial/failed run: drop from active + best-effort teardown.
            self.active.lock().await.remove(&project);
            let _ = self.destroy_project(&project).await;
        }
        result
    }

    async fn create_run_inner(
        &self,
        run_id: &str,
        req: &CreateRunRequest,
        project: String,
    ) -> Result<RunInfo> {
        let git_ref = req.git_ref.clone().unwrap_or_else(|| self.cfg.default_ref.clone());
        let images = resolve_images(&self.cfg, &git_ref).await?;

        // Ephemeral per-run project (deleted with the run).
        self.create_project(&project).await.ok();
        let backend = self.backend(&project)?;

        let servers_req = if req.servers.is_empty() {
            vec![ServerSpec {
                name: "test-mac-mgmt-server".into(),
                mode: "monolith".into(),
                relays: 1,
            }]
        } else {
            req.servers.clone()
        };

        let mut servers = Vec::new();
        for spec in &servers_req {
            // Instance name == expected hostname so peers resolve it via the
            // project network's built-in DNS (mirrors compose network aliases).
            let server_name = spec.name.clone();
            let mut config = serde_json::Map::new();
            config.insert("security.nesting".into(), serde_json::json!("true"));
            if spec.mode != "monolith" {
                config.insert(
                    "environment.MAC_MGMT_SERVER_MODE".into(),
                    serde_json::json!(spec.mode),
                );
            }
            backend
                .launch_ext(&LaunchSpec {
                    name: server_name.clone(),
                    image_alias: image_ref(&images.server_image),
                    image_server: Some(registry_server(&images.server_image)),
                    protocol: Some("oci".into()),
                    instance_type: Some("container".into()),
                    ephemeral: false,
                    profiles: vec!["default".into()],
                    config: config.clone(),
                    ready_timeout_secs: Some(self.cfg.instance_ready_timeout_secs),
                })
                .await
                .with_context(|| format!("launching server {server_name}"))?;

            let mut relays = Vec::new();
            for r in 0..spec.relays {
                let relay_name = if spec.relays == 1 {
                    "test-mac-mgmt-relay".to_string()
                } else {
                    format!("test-mac-mgmt-relay-{r}")
                };
                let mut rcfg = serde_json::Map::new();
                rcfg.insert(
                    "environment.MAC_MGMT_SERVER_API_URL".into(),
                    serde_json::json!(format!("https://{server_name}")),
                );
                backend
                    .launch_ext(&LaunchSpec {
                        name: relay_name.clone(),
                        image_alias: image_ref(&images.relay_image),
                        image_server: Some(registry_server(&images.relay_image)),
                        protocol: Some("oci".into()),
                        instance_type: Some("container".into()),
                        ephemeral: false,
                        profiles: vec!["default".into()],
                        config: rcfg,
                        ready_timeout_secs: Some(self.cfg.instance_ready_timeout_secs),
                    })
                    .await
                    .with_context(|| format!("launching relay {relay_name}"))?;
                relays.push(RelayEndpoint {
                    name: relay_name.clone(),
                    addr: instance_addr(&backend, &relay_name).await,
                });
            }
            servers.push(ServerEndpoint {
                name: server_name.clone(),
                addr: instance_addr(&backend, &server_name).await,
                relays,
            });
        }

        let state = RunState {
            run_id: run_id.to_string(),
            project: project.clone(),
            image_tag: images.short_sha.clone(),
            servers: servers.clone(),
            node_instances: Vec::new(),
            status: "running".into(),
        };
        self.persist(&state)?;

        Ok(RunInfo {
            run_id: run_id.to_string(),
            image_tag: images.short_sha,
            servers,
            instances: Vec::new(),
            status: "running".into(),
        })
    }

    /// Launch `count` daemon instances with the cluster sync token. Returns the
    /// container names; the caller maps them to instance_ids via the server's
    /// machine list.
    pub async fn spawn_nodes(&self, run_id: &str, req: &SpawnNodesRequest) -> Result<Vec<String>> {
        let mut state = self.load_state(run_id)?;
        let backend = self.backend(&state.project)?;
        let images = resolve_images(&self.cfg, &self.cfg.default_ref).await?;
        let mut names = Vec::new();
        for _ in 0..req.count {
            // Pre-generate the host key so the daemon has a stable identity.
            let hk = hostkey::generate()?;
            let name = format!("test-mac-mgmt-daemon-{}", &hk.instance_id[..12]);
            let mut config = serde_json::Map::new();
            config.insert("security.nesting".into(), serde_json::json!("true"));
            backend
                .launch_ext(&LaunchSpec {
                    name: name.clone(),
                    image_alias: image_ref(&images.daemon_image),
                    image_server: Some(registry_server(&images.daemon_image)),
                    protocol: Some("oci".into()),
                    instance_type: Some("container".into()),
                    ephemeral: false,
                    profiles: vec!["default".into()],
                    config,
                    ready_timeout_secs: Some(self.cfg.instance_ready_timeout_secs),
                })
                .await
                .with_context(|| format!("launching daemon {name}"))?;

            // Inject the sync token + pre-generated host key, then (re)start the
            // daemon so it enrolls with the known instance_id.
            backend
                .file_push(&name, "/etc/mac-mgmt.env", format!("MAC_MGMT_TOKEN={}\n", req.sync_token).as_bytes())
                .await
                .ok();
            backend
                .file_push(&name, "/root/.config/mac-mgmt/host_ed25519_key", hk.private_pem.as_bytes())
                .await
                .ok();
            let _ = backend
                .exec(&name, "systemctl restart mac-mgmt", std::time::Duration::from_secs(30))
                .await;

            names.push(name);
            state.node_instances.push(hk.instance_id);
        }
        self.persist(&state)?;
        Ok(names)
    }

    pub async fn delete_run(&self, run_id: &str) -> Result<()> {
        let project = self.project_name(run_id);
        // Force-delete every instance in the project, then the project itself.
        let _ = self.destroy_project(&project).await;
        self.active.lock().await.remove(&project);
        if let Ok(mut s) = self.load_state(run_id) {
            s.status = "torn_down".into();
            let _ = self.persist(&s);
        }
        Ok(())
    }

    /// Garbage-collect stale incus projects: any project whose name starts with
    /// `<incus_project_prefix>-` but isn't held by a live run in this process.
    /// Catches orphans from crashed/abandoned runs (and, on startup when the
    /// active set is empty, everything left over from a previous lifecycle).
    pub async fn gc_projects(&self) -> Result<usize> {
        let prefix = format!(
            "{}-",
            self.cfg.incus_project_prefix.as_deref().unwrap_or("mmrc")
        );
        let active = self.active.lock().await.clone();
        let mgmt = self.backend("default")?;
        let projects = mgmt.project_names().await.context("listing projects")?;
        let mut reaped = 0;
        for project in projects {
            if project.starts_with(&prefix) && !active.contains(&project) {
                tracing::info!("gc: reaping stale project {project}");
                if let Err(e) = self.destroy_project(&project).await {
                    tracing::warn!("gc: failed to reap {project}: {e:#}");
                } else {
                    reaped += 1;
                }
            }
        }
        Ok(reaped)
    }

    pub fn load_state(&self, run_id: &str) -> Result<RunState> {
        let path = self.run_dir(run_id).join("state.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading run state {}", path.display()))?;
        serde_json::from_str(&text).context("parsing run state")
    }

    fn persist(&self, state: &RunState) -> Result<()> {
        let dir = self.run_dir(&state.run_id);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("state.json");
        std::fs::write(&path, serde_json::to_vec_pretty(state)?)
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

/// Extract the `namespace/image:tag` part of a registry ref (drop the host).
fn image_ref(full: &str) -> String {
    full.splitn(2, '/')
        .nth(1)
        .unwrap_or(full)
        .to_string()
}

/// Extract the `https://host` server part of a registry ref.
fn registry_server(full: &str) -> String {
    let host = full.split('/').next().unwrap_or(full);
    format!("https://{host}")
}

/// Host-reachable instance address: the instance's public (global-scope) IPv6,
/// in bracketed URL form (e.g. "[2a01:...]"). Polls the instance state for up to
/// ~60s (SLAAC/DHCPv6 assignment can lag the Running state); falls back to the
/// instance name if none appears.
async fn instance_addr(backend: &Arc<dyn IncusBackend>, name: &str) -> String {
    for _ in 0..30 {
        if let Ok(Some(state)) = backend.instance_state(name).await {
            if let Some(ip) = global_ipv6(&state) {
                return format!("[{ip}]");
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    tracing::warn!("no global IPv6 for instance {name}; falling back to name");
    name.to_string()
}

/// Pick the first global-scope IPv6 across the instance's non-loopback
/// interfaces from `GET /1.0/instances/<name>/state` metadata.
fn global_ipv6(state: &serde_json::Value) -> Option<String> {
    let networks = state.get("network")?.as_object()?;
    for (iface, data) in networks {
        if iface == "lo" {
            continue;
        }
        let Some(addrs) = data.get("addresses").and_then(|a| a.as_array()) else {
            continue;
        };
        for a in addrs {
            let family = a.get("family").and_then(|v| v.as_str()).unwrap_or("");
            let scope = a.get("scope").and_then(|v| v.as_str()).unwrap_or("");
            let address = a.get("address").and_then(|v| v.as_str()).unwrap_or("");
            if family == "inet6" && scope == "global" && !address.is_empty() {
                return Some(address.to_string());
            }
        }
    }
    None
}
