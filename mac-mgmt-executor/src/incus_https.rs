use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use reqwest::Certificate;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

use crate::backend::IncusBackend;
use crate::types::{ExecOutput, OsImage};

pub struct HttpsBackend {
    http: reqwest::Client,
    base: String,
    project: String,
    image_server: String,
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

impl HttpsBackend {
    pub fn from_env(project: Option<String>) -> Result<Self> {
        let base = std::env::var("INCUS_URL").context("INCUS_URL not set")?;
        let cert_path =
            std::env::var("INCUS_CLIENT_CERT").context("INCUS_CLIENT_CERT not set")?;
        let key_path =
            std::env::var("INCUS_CLIENT_KEY").context("INCUS_CLIENT_KEY not set")?;

        let cert_pem = std::fs::read(&cert_path)
            .with_context(|| format!("reading client cert: {cert_path}"))?;
        let key_pem = std::fs::read(&key_path)
            .with_context(|| format!("reading client key: {key_path}"))?;

        let mut combined = cert_pem;
        combined.push(b'\n');
        combined.extend_from_slice(&key_pem);

        let identity = reqwest::Identity::from_pem(&combined)
            .context("parsing client cert+key PEM")?;

        let mut builder = reqwest::Client::builder().identity(identity);

        if let Ok(ca_path) = std::env::var("INCUS_SERVER_CA") {
            let ca_pem = std::fs::read(&ca_path)
                .with_context(|| format!("reading server CA: {ca_path}"))?;
            builder = builder.add_root_certificate(
                Certificate::from_pem(&ca_pem).context("parsing server CA PEM")?,
            );
        } else {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let http = builder.build().context("building HTTPS client")?;

        let image_server = std::env::var("INCUS_IMAGE_SERVER")
            .unwrap_or_else(|_| "https://images.linuxcontainers.org".to_string());

        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            project: project.unwrap_or_else(|| "default".to_string()),
            image_server,
        })
    }

    fn url(&self, path: &str) -> String {
        if path.contains('?') {
            format!("{}{}&project={}", self.base, path, self.project)
        } else {
            format!("{}{}?project={}", self.base, path, self.project)
        }
    }

    async fn send_and_unwrap(&self, req: reqwest::RequestBuilder) -> Result<Value> {
        let resp = req.send().await.context("sending Incus request")?;
        let status = resp.status();
        let env: Envelope = resp.json().await.context("decoding Incus response")?;

        if !status.is_success() && env.status_code.unwrap_or_default() / 100 != 2 {
            bail!(
                "Incus error: status={status} err={:?}",
                env.error,
            );
        }

        match env.kind.as_str() {
            "sync" => Ok(env.metadata.unwrap_or(Value::Null)),
            "async" => {
                let op = env
                    .operation
                    .context("async response had no operation URL")?;
                let op_url = format!("{}{}/wait?timeout=120", self.base, op);
                let wait = self
                    .http
                    .get(op_url)
                    .send()
                    .await
                    .context("awaiting Incus operation")?;
                let wait_status = wait.status();
                let wait_env: Envelope =
                    wait.json().await.context("decoding op envelope")?;
                if !wait_status.is_success() || wait_env.status_code != Some(200) {
                    bail!(
                        "Incus async op failed: status={wait_status} err={:?}",
                        wait_env.error
                    );
                }
                Ok(wait_env.metadata.unwrap_or(Value::Null))
            }
            other => bail!("unknown Incus envelope type: {other}"),
        }
    }
}

#[async_trait]
impl IncusBackend for HttpsBackend {
    async fn launch(&self, image: &str, name: &str) -> Result<()> {
        let body = json!({
            "name": name,
            "type": "container",
            "ephemeral": true,
            "source": {
                "type": "image",
                "protocol": "simplestreams",
                "server": self.image_server,
                "alias": image,
            },
            "profiles": ["default"],
            "start": true,
        });

        self.send_and_unwrap(
            self.http.post(self.url("/1.0/instances")).json(&body),
        )
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
        // The Incus REST exec API requires websocket handling for I/O,
        // which is complex. Fall back to CLI exec for the HTTPS backend too.
        let mut args = vec![
            "exec".to_string(),
            name.to_string(),
            "--".to_string(),
            "sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ];

        // If we have a project, pass it.
        if self.project != "default" {
            args.insert(1, "--project".to_string());
            args.insert(2, self.project.clone());
        }

        let result = tokio::time::timeout(
            timeout,
            tokio::process::Command::new("incus")
                .args(&args)
                .stdin(std::process::Stdio::null())
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
        // Stop first (ignore errors -- may already be stopped).
        let stop_body = json!({
            "action": "stop",
            "timeout": 30,
            "force": true,
        });
        let _ = self
            .send_and_unwrap(
                self.http
                    .put(self.url(&format!("/1.0/instances/{name}/state")))
                    .json(&stop_body),
            )
            .await;

        let resp = self
            .http
            .delete(self.url(&format!("/1.0/instances/{name}")))
            .send()
            .await
            .context("delete instance")?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(());
        }

        let env: Envelope = resp.json().await.context("decoding delete envelope")?;
        if env.kind == "async" {
            if let Some(op) = env.operation {
                let op_url = format!("{}{}/wait?timeout=120", self.base, op);
                let _ = self.http.get(op_url).send().await;
            }
        }
        if !status.is_success() && env.status_code.unwrap_or_default() / 100 != 2 {
            bail!("delete failed: status={status} err={:?}", env.error);
        }
        Ok(())
    }

    async fn status(&self, name: &str) -> Result<Option<String>> {
        let resp = self
            .http
            .get(self.url(&format!("/1.0/instances/{name}/state")))
            .send()
            .await
            .context("fetching instance state")?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        let env: Envelope = resp.json().await.context("decoding state envelope")?;
        Ok(env
            .metadata
            .as_ref()
            .and_then(|m| m.get("status"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()))
    }

    async fn image_list(&self) -> Result<Vec<OsImage>> {
        // Use the simplestreams index to list images.
        // This is the same source `incus image list images:` uses.
        let url = format!("{}/streams/v1/images.json", self.image_server);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("fetching image index from simplestreams")?;

        if !resp.status().is_success() {
            bail!("failed to fetch image index: status={}", resp.status());
        }

        let body: Value = resp.json().await.context("decoding image index")?;

        let current_arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        };

        let mut images = Vec::new();

        if let Some(products) = body.get("products").and_then(|p| p.as_object()) {
            for (_key, product) in products {
                let arch = product
                    .get("arch")
                    .and_then(|a| a.as_str())
                    .unwrap_or("");
                if arch != current_arch {
                    continue;
                }

                // Filter to container (lxd.tar.xz) types.
                let ftype = product
                    .get("ftype")
                    .and_then(|f| f.as_str())
                    .unwrap_or("");
                if ftype == "disk-kvm.img" || ftype == "disk1.img" {
                    continue;
                }

                let os = product
                    .get("os")
                    .and_then(|o| o.as_str())
                    .unwrap_or("")
                    .to_string();
                let release = product
                    .get("release")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                let variant = product
                    .get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                // Build alias from key: e.g. "ubuntu:24.04:amd64:default" -> "ubuntu/24.04"
                let alias = if variant == "default" {
                    format!("{}/{}", os.to_lowercase(), release)
                } else {
                    format!("{}/{}/{}", os.to_lowercase(), release, variant)
                };

                let description = format!("{os} {release} {arch}");

                images.push(OsImage {
                    alias,
                    description,
                    os,
                    release,
                    variant,
                    image_type: "container".to_string(),
                });
            }
        }

        images.sort_by(|a, b| a.alias.cmp(&b.alias));
        images.dedup_by(|a, b| a.alias == b.alias);

        Ok(images)
    }

    async fn file_push(&self, name: &str, path: &str, content: &[u8]) -> Result<()> {
        let url = self.url(&format!(
            "/1.0/instances/{name}/files?path={path}"
        ));
        let resp = self
            .http
            .post(&url)
            .header("X-Incus-type", "file")
            .body(content.to_vec())
            .send()
            .await
            .context("file push request")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("file push failed: {body}");
        }
        Ok(())
    }

    async fn file_pull(&self, name: &str, path: &str) -> Result<String> {
        let url = self.url(&format!(
            "/1.0/instances/{name}/files?path={path}"
        ));
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("file pull request")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("file pull failed: {body}");
        }

        let bytes = resp.bytes().await.context("reading file content")?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}
