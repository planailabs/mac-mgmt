use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use reqwest::Certificate;
use serde_json::Value;
use std::time::Duration;

use crate::backend::IncusBackend;
use crate::incus_common::{
    self, Envelope, append_project, exec_via_cli, extract_status, launch_body, stop_body,
    wait_for_running,
};
use crate::types::{ExecOutput, OsImage};

/// Backend that talks to the Incus daemon over HTTPS with mTLS.
pub struct HttpsBackend {
    http: reqwest::Client,
    base: String,
    project: String,
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

        let identity =
            reqwest::Identity::from_pem(&combined).context("parsing client cert+key PEM")?;

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

        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            project: project.unwrap_or_else(|| "default".to_string()),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, append_project(path, &self.project))
    }

    async fn send_and_unwrap(&self, req: reqwest::RequestBuilder) -> Result<Value> {
        let resp = req.send().await.context("sending Incus request")?;
        let status = resp.status();
        let env: Envelope = resp.json().await.context("decoding Incus response")?;

        if !status.is_success() && env.status_code.unwrap_or_default() / 100 != 2 {
            bail!("Incus error: status={status} err={:?}", env.error);
        }

        match env.kind.as_str() {
            "sync" => Ok(env.metadata.unwrap_or(Value::Null)),
            "async" => {
                let op = env
                    .operation
                    .context("async response had no operation URL")?;
                let op_url = format!("{}{}/wait?timeout=120", self.base, op);
                let wait = self.http.get(op_url).send().await.context("awaiting op")?;
                let wait_status = wait.status();
                let wait_env: Envelope = wait.json().await.context("decoding op envelope")?;
                if !wait_status.is_success() || wait_env.status_code != Some(200) {
                    bail!("Incus async op failed: {:?}", wait_env.error);
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
        let body = launch_body(image, name);
        self.send_and_unwrap(
            self.http.post(self.url("/1.0/instances")).json(&body),
        )
        .await?;
        wait_for_running(self, name).await
    }

    async fn exec(&self, name: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        exec_via_cli(name, command, timeout, &self.project).await
    }

    async fn delete(&self, name: &str) -> Result<()> {
        let _ = self
            .send_and_unwrap(
                self.http
                    .put(self.url(&format!("/1.0/instances/{name}/state")))
                    .json(&stop_body()),
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
        Ok(extract_status(&env.metadata))
    }

    async fn image_list(&self) -> Result<Vec<OsImage>> {
        incus_common::image_list_via_cli().await
    }

    async fn file_push(&self, name: &str, path: &str, content: &[u8]) -> Result<()> {
        let resp = self
            .http
            .post(self.url(&format!("/1.0/instances/{name}/files?path={path}")))
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
        let resp = self
            .http
            .get(self.url(&format!("/1.0/instances/{name}/files?path={path}")))
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
