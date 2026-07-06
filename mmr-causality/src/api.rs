//! Shared request/response types for the mmrcd HTTP API. Used by both `mmrcd`
//! (server) and `mmrc`'s antithesis env (client).

use serde::{Deserialize, Serialize};

/// A server instance to spawn in a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSpec {
    pub name: String,
    /// "monolith" (default) or "skill-center".
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Number of relays attached to this server (default 1 for the primary).
    #[serde(default = "default_relays")]
    pub relays: usize,
}

fn default_mode() -> String {
    "monolith".into()
}
fn default_relays() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunRequest {
    /// Git ref (branch/commit) to resolve images for. Defaults to trunk.
    #[serde(default)]
    pub git_ref: Option<String>,
    #[serde(default)]
    pub servers: Vec<ServerSpec>,
    #[serde(default)]
    pub nodes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayEndpoint {
    pub name: String,
    /// Host-reachable address (e.g. "http://127.0.0.1:PORT" or an instance IP).
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEndpoint {
    pub name: String,
    pub addr: String,
    pub relays: Vec<RelayEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    pub run_id: String,
    pub image_tag: String,
    pub servers: Vec<ServerEndpoint>,
    #[serde(default)]
    pub instances: Vec<String>,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnNodesRequest {
    pub count: usize,
    /// "fleet" or "chaos".
    pub kind: String,
    pub cluster_id: uuid::Uuid,
    pub sync_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnNodesResponse {
    pub instance_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageStatus {
    pub git_ref: String,
    pub short_sha: String,
    pub pipeline_status: String,
    pub server_image: String,
    pub relay_image: String,
    pub daemon_image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadModelRequest {
    /// Path on the mmrcd host to the GGUF file.
    pub gguf_path: String,
    pub modelfile: String,
    pub model_name: String,
}
