use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::backend::IncusBackend;
use crate::incus_common::{
    Envelope, append_project, exec_via_cli, extract_status, image_list_via_cli, is_not_found,
    OP_WAIT_SECS, launch_body, launch_body_ext, stop_body, wait_for_running,
};
use crate::types::{ExecOutput, LaunchSpec, OsImage};

const DEFAULT_SOCKET: &str = "/var/lib/incus/unix.socket";

/// Backend that talks to the Incus daemon over its Unix socket.
pub struct UnixBackend {
    socket_path: PathBuf,
    project: String,
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
        append_project(path, &self.project)
    }

    async fn request(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<Envelope> {
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

        serde_json::from_str(&json_body)
            .with_context(|| format!("parsing Incus response for {method} {path}"))
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

    async fn send_and_unwrap(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let env = self.request_json(method, path, body).await?;

        if env.status_code.unwrap_or_default() / 100 != 2 && env.kind != "async" {
            bail!("Incus error on {method} {path}: {:?}", env.error);
        }

        match env.kind.as_str() {
            "sync" => Ok(env.metadata.unwrap_or(Value::Null)),
            "async" => {
                let op = env
                    .operation
                    .context("async response had no operation URL")?;
                let wait_env = self
                    .request("GET", &format!("{op}/wait?timeout={OP_WAIT_SECS}"), None)
                    .await?;
                if wait_env.status_code != Some(200) {
                    bail!("Incus async op failed: {:?}", wait_env.error);
                }
                Ok(wait_env.metadata.unwrap_or(Value::Null))
            }
            other => bail!("unknown Incus envelope type: {other}"),
        }
    }

    async fn get_raw(&self, path: &str) -> Result<Vec<u8>> {
        let url = self.url(path);
        let request = format!("GET {url} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");

        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(request.as_bytes()).await?;

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;

        if let Some(pos) = find_header_end(&buf) {
            let headers = String::from_utf8_lossy(&buf[..pos]);
            let body = &buf[pos..];
            if headers.contains("chunked") {
                Ok(decode_chunked(&String::from_utf8_lossy(body)).into_bytes())
            } else {
                Ok(body.to_vec())
            }
        } else {
            Ok(buf)
        }
    }

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
        let body = launch_body(image, name);
        self.send_and_unwrap("POST", "/1.0/instances", Some(&body))
            .await?;
        wait_for_running(self, name, Duration::from_secs(60)).await
    }

    async fn launch_ext(&self, spec: &LaunchSpec) -> Result<()> {
        let body = launch_body_ext(spec);
        self.send_and_unwrap("POST", "/1.0/instances", Some(&body))
            .await?;
        let timeout = Duration::from_secs(spec.ready_timeout_secs.unwrap_or(60));
        wait_for_running(self, &spec.name, timeout).await
    }

    async fn exec(&self, name: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        exec_via_cli(name, command, timeout, &self.project).await
    }

    async fn delete(&self, name: &str) -> Result<()> {
        let _ = self
            .send_and_unwrap(
                "PUT",
                &format!("/1.0/instances/{name}/state"),
                Some(&stop_body()),
            )
            .await;

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
            Ok(e) if is_not_found(&e) => {}
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
        match self
            .request("GET", &format!("/1.0/instances/{name}/state"), None)
            .await
        {
            Ok(e) => Ok(extract_status(&e.metadata)),
            Err(_) => Ok(None),
        }
    }

    async fn instance_state(&self, name: &str) -> Result<Option<serde_json::Value>> {
        match self
            .request("GET", &format!("/1.0/instances/{name}/state"), None)
            .await
        {
            Ok(e) => Ok(e.metadata),
            Err(_) => Ok(None),
        }
    }

    async fn image_list(&self) -> Result<Vec<OsImage>> {
        image_list_via_cli().await
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

    async fn create_project(
        &self,
        name: &str,
        config: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        let body = crate::incus_common::project_body(name, &config);
        self.send_and_unwrap("POST", "/1.0/projects", Some(&body))
            .await?;
        Ok(())
    }

    async fn delete_project(&self, name: &str) -> Result<()> {
        self.send_and_unwrap("DELETE", &format!("/1.0/projects/{name}"), None)
            .await?;
        Ok(())
    }

    async fn project_instance_names(&self) -> Result<Vec<String>> {
        let meta = self.send_and_unwrap("GET", "/1.0/instances", None).await?;
        Ok(crate::incus_common::parse_instance_names(&meta))
    }

    async fn project_names(&self) -> Result<Vec<String>> {
        let meta = self.send_and_unwrap("GET", "/1.0/projects", None).await?;
        Ok(crate::incus_common::parse_instance_names(&meta))
    }
}

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

fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}
