use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::process::Command;

use crate::backend::IncusBackend;
use crate::types::{ExecOutput, OsImage};

/// Incus REST API envelope returned by all endpoints.
#[derive(Debug, Deserialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub status_code: Option<i64>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Default image server for simplestreams.
pub const IMAGE_SERVER: &str = "https://images.linuxcontainers.org";

/// Build the JSON body for creating an ephemeral container.
pub fn launch_body(image: &str, name: &str) -> Value {
    json!({
        "name": name,
        "type": "container",
        "ephemeral": true,
        "source": {
            "type": "image",
            "protocol": "simplestreams",
            "server": IMAGE_SERVER,
            "alias": image,
        },
        "profiles": ["default"],
        "start": true,
    })
}

/// Build the JSON body for force-stopping a container.
pub fn stop_body() -> Value {
    json!({
        "action": "stop",
        "timeout": 30,
        "force": true,
    })
}

/// Poll `backend.status()` until the container is "Running" (up to 60s).
pub async fn wait_for_running(backend: &dyn IncusBackend, name: &str) -> Result<()> {
    for _ in 0..60 {
        if let Some(s) = backend.status(name).await? {
            if s == "Running" {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    bail!("container {name} did not reach Running state within 60s");
}

/// Extract status string from Incus state metadata.
pub fn extract_status(metadata: &Option<Value>) -> Option<String> {
    metadata
        .as_ref()
        .and_then(|m| m.get("status"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Check whether an Envelope represents a "not found" error.
pub fn is_not_found(env: &Envelope) -> bool {
    env.status_code == Some(404)
        || env
            .error
            .as_deref()
            .is_some_and(|s| s.contains("not found") || s.contains("Not found"))
}

/// Execute a command inside a container via the `incus exec` CLI.
///
/// Both the Unix and HTTPS backends use this because the REST exec API
/// requires websockets for I/O.
pub async fn exec_via_cli(
    name: &str,
    command: &str,
    timeout: Duration,
    project: &str,
) -> Result<ExecOutput> {
    let mut args = vec!["exec".to_string()];
    if project != "default" {
        args.push("--project".to_string());
        args.push(project.to_string());
    }
    args.extend([
        name.to_string(),
        "--".to_string(),
        "sh".to_string(),
        "-c".to_string(),
        command.to_string(),
    ]);

    let result = tokio::time::timeout(
        timeout,
        Command::new("incus")
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => Ok(ExecOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        }),
        Ok(Err(e)) => bail!("failed to spawn incus exec: {e}"),
        Err(_) => bail!("command timed out after {}s", timeout.as_secs()),
    }
}

/// Fetch image list via the `incus image list` CLI.
pub async fn image_list_via_cli() -> Result<Vec<OsImage>> {
    let output = Command::new("incus")
        .args(["image", "list", "images:", "--format", "json"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("incus image list failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<Value> = serde_json::from_str(&stdout)?;
    Ok(parse_image_list(&entries))
}

/// Parse the JSON array returned by `incus image list images: --format json`.
/// Filters to container-type images matching the current architecture.
pub fn parse_image_list(entries: &[Value]) -> Vec<OsImage> {
    let incus_arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };

    let mut images = Vec::new();
    for entry in entries {
        let props = match entry.get("properties") {
            Some(p) => p,
            None => continue,
        };

        let arch = props
            .get("architecture")
            .and_then(|a| a.as_str())
            .unwrap_or("");
        if arch != incus_arch {
            continue;
        }

        let image_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if image_type != "container" {
            continue;
        }

        let aliases = entry.get("aliases").and_then(|a| a.as_array());
        let alias = aliases
            .and_then(|a| {
                a.iter()
                    .filter_map(|e| e.get("name").and_then(|n| n.as_str()))
                    .min_by_key(|n| n.len())
            })
            .unwrap_or("");

        if alias.is_empty() {
            continue;
        }

        images.push(OsImage {
            alias: alias.to_string(),
            description: props
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string(),
            os: props
                .get("os")
                .and_then(|o| o.as_str())
                .unwrap_or("")
                .to_string(),
            release: props
                .get("release")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string(),
            variant: props
                .get("variant")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string(),
            image_type: image_type.to_string(),
        });
    }

    images.sort_by(|a, b| a.alias.cmp(&b.alias));
    images.dedup_by(|a, b| a.alias == b.alias);
    images
}

/// Append `?project=<p>` or `&project=<p>` to a URL path.
pub fn append_project(path: &str, project: &str) -> String {
    if path.contains('?') {
        format!("{path}&project={project}")
    } else {
        format!("{path}?project={project}")
    }
}
