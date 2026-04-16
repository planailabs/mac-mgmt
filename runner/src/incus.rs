//! Minimal Incus REST API client.
//!
//! Authenticates with a PEM client certificate/key and talks to the Incus
//! HTTPS endpoint. Handles Incus's sync/async operation model: async ops are
//! polled via `/1.0/operations/<id>/wait` before the call returns.

use anyhow::{Context, Result, bail};
use reqwest::Certificate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub struct IncusClient {
    http: reqwest::Client,
    base: String,
    project: String,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    status_code: Option<i64>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    metadata: Option<Value>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateInstanceSpec {
    pub name: String,
    pub instance_type: String,
    pub image_alias: String,
    pub image_server: String,
    pub profiles: Vec<String>,
    /// The cloud-init YAML body.
    pub cloud_init_user_data: String,
}

impl IncusClient {
    pub fn new(
        base: &str,
        project: &str,
        client_pem_combined: &[u8],
        server_ca: Option<&[u8]>,
    ) -> Result<Self> {
        let mut builder = reqwest::Client::builder()
            .identity(reqwest::Identity::from_pem(client_pem_combined)
                .context("parsing Incus client identity (cert+key PEM)")?);
        match server_ca {
            Some(ca) => {
                builder = builder.add_root_certificate(
                    Certificate::from_pem(ca).context("parsing Incus server CA")?,
                );
            }
            None => {
                builder = builder.danger_accept_invalid_certs(true);
            }
        }
        let http = builder.build().context("building Incus http client")?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            project: project.to_string(),
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
                "Incus error: status={status} err={:?} status_msg={:?}",
                env.error,
                env.status
            );
        }

        match env.kind.as_str() {
            "sync" => Ok(env.metadata.unwrap_or(Value::Null)),
            "async" => {
                let op = env.operation.context("async response had no operation URL")?;
                let op_url = format!("{}{}/wait?timeout=120", self.base, op);
                let wait = self
                    .http
                    .get(op_url)
                    .send()
                    .await
                    .context("awaiting Incus operation")?;
                let wait_status = wait.status();
                let wait_env: Envelope = wait.json().await.context("decoding op envelope")?;
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

    pub async fn create_instance(&self, spec: &CreateInstanceSpec) -> Result<()> {
        let body = json!({
            "name": spec.name,
            "type": spec.instance_type,
            "source": {
                "type": "image",
                "protocol": "simplestreams",
                "server": spec.image_server,
                "alias": spec.image_alias,
            },
            "profiles": spec.profiles,
            "config": {
                "cloud-init.user-data": spec.cloud_init_user_data,
                "security.nesting": "true",
            },
            "start": true,
        });
        self.send_and_unwrap(self.http.post(self.url("/1.0/instances")).json(&body))
            .await?;
        Ok(())
    }

    pub async fn set_instance_state(&self, name: &str, action: &str) -> Result<()> {
        let body = json!({
            "action": action,
            "timeout": 60,
            "force": action == "stop",
        });
        self.send_and_unwrap(
            self.http
                .put(self.url(&format!("/1.0/instances/{name}/state")))
                .json(&body),
        )
        .await?;
        Ok(())
    }

    pub async fn stop_instance(&self, name: &str) -> Result<()> {
        // Ignore "already stopped" style failures — instance may be absent.
        if let Err(e) = self.set_instance_state(name, "stop").await {
            tracing::debug!("stop_instance({name}) soft-failed: {e}");
        }
        Ok(())
    }

    pub async fn delete_instance(&self, name: &str) -> Result<()> {
        // Incus requires stopped state before delete.
        let _ = self.stop_instance(name).await;
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
            bail!("delete_instance({name}): status={status} err={:?}", env.error);
        }
        Ok(())
    }

    /// Names of every instance in the configured project. Incus returns
    /// entries as URLs like `/1.0/instances/<name>?project=<p>`.
    pub async fn list_instances(&self) -> Result<Vec<String>> {
        let v = self
            .send_and_unwrap(self.http.get(self.url("/1.0/instances")))
            .await?;
        let urls: Vec<&str> = v
            .as_array()
            .map(|a| a.iter().filter_map(|u| u.as_str()).collect())
            .unwrap_or_default();
        Ok(parse_instance_urls(&urls, &self.project))
    }

    #[allow(dead_code)]
    pub async fn instance_exists(&self, name: &str) -> Result<bool> {
        let resp = self
            .http
            .get(self.url(&format!("/1.0/instances/{name}")))
            .send()
            .await
            .context("checking instance existence")?;
        Ok(resp.status().is_success())
    }
}

/// Split `GET /1.0/instances` URL list into instance names, dropping any
/// entry whose `project=` hint doesn't match `project`. Pulled out so we
/// can regression-test the parsing without a live Incus host.
fn parse_instance_urls(urls: &[&str], project: &str) -> Vec<String> {
    let mut out = Vec::new();
    for u in urls {
        let (path, query) = match u.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (*u, None),
        };
        if let Some(q) = query {
            let p = q
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .find(|(k, _)| *k == "project")
                .map(|(_, v)| v);
            if let Some(p) = p {
                if p != project {
                    continue;
                }
            }
        }
        if let Some(name) = path.rsplit('/').next() {
            if !name.is_empty() {
                out.push(name.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_instances_strips_query_and_filters_project() {
        let urls = vec![
            "/1.0/instances/test-mmr-openclaw-lms?project=test-mmr",
            "/1.0/instances/test-mmr-openclaw-ollama?project=test-mmr",
            "/1.0/instances/other-project-vm?project=other",
            "/1.0/instances/no-query-name",
            "/1.0/instances/multi-arg?project=test-mmr&foo=bar",
        ];
        let got = parse_instance_urls(&urls, "test-mmr");
        assert_eq!(
            got,
            vec![
                "test-mmr-openclaw-lms",
                "test-mmr-openclaw-ollama",
                "no-query-name",
                "multi-arg",
            ]
        );
    }
}
