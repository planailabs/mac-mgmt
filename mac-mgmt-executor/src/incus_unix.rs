use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::backend::IncusBackend;
use crate::types::{ExecOutput, OsImage};

const DEFAULT_SOCKET: &str = "/var/lib/incus/unix.socket";
const IMAGE_SERVER: &str = "https://images.linuxcontainers.org";

/// Backend that talks to the Incus daemon over its Unix socket.
pub struct UnixBackend {
    socket_path: PathBuf,
    project: String,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    status_code: Option<i64>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    metadata: Option<Value>,
    #[serde(default)]
    error: Option<String>,
}

impl UnixBackend {
    pub fn new(project: Option<String>) -> Self {
        let socket_path = std::env::var("INCUS_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_SOCKET));
        Self {
            socket_path,
            project: project.unwrap_or_else(|| "default".to_string()),
        }
    }

    fn url(&self, path: &str) -> String {
        if path.contains('?') {
            format!("{path}&project={}", self.project)
        } else {
            format!("{path}?project={}", self.project)
        }
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Envelope> {
        let url = self.url(path);
        let body_bytes = body.unwrap_or(b"");

        let mut request = format!("{method} {url} HTTP/1.1\r\nHost: localhost\r\n");
        if !body_bytes.is_empty() {
            request.push_str("Content-Type: application/json\r\n");
            request.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
        }
        request.push_str("Connection: close\r\n\r\n");

        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .with_context(|| format!("connecting to {}", self.socket_path.display()))?;

        stream.write_all(request.as_bytes()).await?;
        if !body_bytes.is_empty() {
            stream.write_all(body_bytes).await?;
        }

        // Read response.
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;

        let response = String::from_utf8_lossy(&buf);

        // Split headers from body.
        let body_str = response
            .split_once("\r\n\r\n")
            .map(|(_, b)| b)
            .unwrap_or(&response);

        // Handle chunked transfer encoding: strip chunk size lines.
        let json_body = if body_str.starts_with(|c: char| c.is_ascii_hexdigit()) {
            decode_chunked(body_str)
        } else {
            body_str.to_string()
        };

        serde_json::from_str(&json_body)
            .with_context(|| format!("parsing Incus response for {method} {path}: {json_body}"))
    }

    async fn request_json(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Envelope> {
        let body_bytes = body.map(|v| serde_json::to_vec(v).unwrap());
        self.request(method, path, body_bytes.as_deref()).await
    }

    /// Send a request and handle the Incus sync/async operation model.
    async fn send_and_unwrap(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let env = self.request_json(method, path, body).await?;

        if env.status_code.unwrap_or_default() / 100 != 2
            && env.kind != "async"
        {
            bail!(
                "Incus error on {method} {path}: {:?}",
                env.error,
            );
        }

        match env.kind.as_str() {
            "sync" => Ok(env.metadata.unwrap_or(Value::Null)),
            "async" => {
                let op = env
                    .operation
                    .context("async response had no operation URL")?;
                let wait_path = format!("{op}/wait?timeout=120");
                let wait_env = self.request("GET", &wait_path, None).await?;
                if wait_env.status_code != Some(200) {
                    bail!("Incus async op failed: {:?}", wait_env.error);
                }
                Ok(wait_env.metadata.unwrap_or(Value::Null))
            }
            other => bail!("unknown Incus envelope type: {other}"),
        }
    }

    /// Raw GET returning the full response bytes (for file pull).
    async fn get_raw(&self, path: &str) -> Result<Vec<u8>> {
        let url = self.url(path);
        let request = format!(
            "GET {url} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        );

        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(request.as_bytes()).await?;

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;

        // Split at the blank line between headers and body.
        let response = &buf;
        if let Some(pos) = find_header_end(response) {
            let body = &response[pos..];
            // Check for chunked encoding.
            let headers = String::from_utf8_lossy(&response[..pos]);
            if headers.contains("chunked") {
                Ok(decode_chunked_bytes(body))
            } else {
                Ok(body.to_vec())
            }
        } else {
            Ok(buf)
        }
    }

    /// Raw POST with binary body (for file push).
    async fn post_raw(&self, path: &str, content: &[u8]) -> Result<Envelope> {
        let url = self.url(path);
        let request = format!(
            "POST {url} HTTP/1.1\r\n\
             Host: localhost\r\n\
             Content-Type: application/octet-stream\r\n\
             X-Incus-type: file\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\r\n",
            content.len()
        );

        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(request.as_bytes()).await?;
        stream.write_all(content).await?;

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;

        let response = String::from_utf8_lossy(&buf);
        let body_str = response
            .split_once("\r\n\r\n")
            .map(|(_, b)| b)
            .unwrap_or(&response);
        let json_body = if body_str.starts_with(|c: char| c.is_ascii_hexdigit()) {
            decode_chunked(body_str)
        } else {
            body_str.to_string()
        };

        serde_json::from_str(&json_body).context("parsing file push response")
    }
}

#[async_trait]
impl IncusBackend for UnixBackend {
    async fn launch(&self, image: &str, name: &str) -> Result<()> {
        let body = json!({
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
        });

        self.send_and_unwrap("POST", "/1.0/instances", Some(&body))
            .await?;

        // Wait for container to be running (up to 60s).
        for _ in 0..60 {
            if let Some(s) = self.status(name).await? {
                if s == "Running" {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        bail!("container {name} did not reach Running state within 60s");
    }

    async fn exec(&self, name: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        // The Incus REST exec API requires websockets for I/O.
        // Fall back to the CLI for this operation.
        let mut args = vec!["exec".to_string(), name.to_string()];
        if self.project != "default" {
            args.insert(1, "--project".to_string());
            args.insert(2, self.project.clone());
        }
        args.extend([
            "--".to_string(),
            "sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ]);

        let result = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("incus")
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

    async fn delete(&self, name: &str) -> Result<()> {
        // Stop first.
        let stop_body = json!({
            "action": "stop",
            "timeout": 30,
            "force": true,
        });
        let _ = self
            .send_and_unwrap(
                "PUT",
                &format!("/1.0/instances/{name}/state"),
                Some(&stop_body),
            )
            .await;

        // Delete.
        let env = self
            .request("DELETE", &format!("/1.0/instances/{name}"), None)
            .await;

        match env {
            Ok(e) if e.kind == "async" => {
                if let Some(op) = e.operation {
                    let _ = self
                        .request("GET", &format!("{op}/wait?timeout=60"), None)
                        .await;
                }
            }
            Ok(e)
                if e.status_code == Some(404)
                    || e.error
                        .as_deref()
                        .is_some_and(|s| s.contains("not found") || s.contains("Not found")) =>
            {
                // Already gone (ephemeral containers auto-delete on stop).
            }
            Ok(e) if e.status_code.unwrap_or_default() / 100 != 2 => {
                bail!("delete failed: {:?}", e.error);
            }
            Err(e) => {
                tracing::debug!("delete {name} soft-failed: {e}");
            }
            _ => {}
        }
        Ok(())
    }

    async fn status(&self, name: &str) -> Result<Option<String>> {
        let env = self
            .request("GET", &format!("/1.0/instances/{name}/state"), None)
            .await;

        match env {
            Ok(e) => Ok(e
                .metadata
                .as_ref()
                .and_then(|m| m.get("status"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())),
            Err(_) => Ok(None),
        }
    }

    async fn image_list(&self) -> Result<Vec<OsImage>> {
        // Fetch from the remote image server's simplestreams index.
        // This is the same data `incus image list images:` returns.
        let env = self
            .request(
                "GET",
                "/1.0/images?filter=&public=true&project=default",
                None,
            )
            .await;

        // If local image list fails, fall back to CLI.
        if env.is_err() {
            tracing::debug!("falling back to CLI for image list");
            let output = tokio::process::Command::new("incus")
                .args(["image", "list", "images:", "--format", "json"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await
                .context("failed to spawn incus image list")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let entries: Vec<Value> =
                serde_json::from_str(&stdout).context("parsing image list")?;
            return Ok(crate::incus_common::parse_image_list(&entries));
        }

        // The local images endpoint doesn't include remote images.
        // We need to use the CLI for this.
        tracing::debug!("using CLI for remote image list");
        let output = tokio::process::Command::new("incus")
            .args(["image", "list", "images:", "--format", "json"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .context("failed to spawn incus image list")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<Value> =
            serde_json::from_str(&stdout).context("parsing image list")?;
        Ok(crate::incus_common::parse_image_list(&entries))
    }

    async fn file_push(&self, name: &str, path: &str, content: &[u8]) -> Result<()> {
        let env = self
            .post_raw(&format!("/1.0/instances/{name}/files?path={path}"), content)
            .await?;

        if env.status_code.unwrap_or_default() / 100 != 2 {
            bail!("file push failed: {:?}", env.error);
        }
        Ok(())
    }

    async fn file_pull(&self, name: &str, path: &str) -> Result<String> {
        let bytes = self
            .get_raw(&format!("/1.0/instances/{name}/files?path={path}"))
            .await?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

/// Decode a chunked transfer-encoded string body.
fn decode_chunked(s: &str) -> String {
    let mut result = String::new();
    let mut rest = s;
    loop {
        let (size_str, after) = match rest.split_once("\r\n") {
            Some(pair) => pair,
            None => break,
        };
        let size = usize::from_str_radix(size_str.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        if after.len() >= size {
            result.push_str(&after[..size]);
            rest = if after.len() > size + 2 {
                &after[size + 2..]
            } else {
                ""
            };
        } else {
            result.push_str(after);
            break;
        }
    }
    result
}

/// Decode chunked transfer encoding from raw bytes.
fn decode_chunked_bytes(data: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(data);
    decode_chunked(&s).into_bytes()
}

/// Find the end of HTTP headers (\r\n\r\n) and return the position
/// of the body start.
fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}
