use anyhow::{Context, Result};
use reqwest::header::{AUTHORIZATION, HeaderValue};

/// HTTP client that talks to the daemon's memvault REST API.
pub struct HttpClient {
    client: reqwest::Client,
    base_url: String,
}

impl HttpClient {
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        let auth_value = format!("Bearer {token}");
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value).context("invalid token")?,
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.base_url)
    }

    // ── Documents ────────────────────────────────────────────────────

    pub async fn put_doc(
        &self,
        body: &str,
        frontmatter: serde_json::Value,
        tags: Vec<(String, String)>,
        visibility: Option<&str>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url("/docs"))
            .json(&serde_json::json!({
                "body": body,
                "frontmatter": frontmatter,
                "tags": tags,
                "visibility": visibility,
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn get_doc(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let resp = self.client.get(self.url(&format!("/docs/{id}"))).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(resp.error_for_status()?.json().await?))
    }

    pub async fn list_docs(
        &self,
        tag_ns: Option<&str>,
        tag_val: Option<&str>,
        limit: usize,
    ) -> Result<serde_json::Value> {
        let mut url = format!("{}?limit={limit}", self.url("/docs"));
        if let Some(ns) = tag_ns {
            url.push_str(&format!("&tag_ns={ns}"));
        }
        if let Some(val) = tag_val {
            url.push_str(&format!("&tag_val={val}"));
        }
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn delete_doc(&self, id: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .delete(self.url(&format!("/docs/{id}")))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    // ── Search ───────────────────────────────────────────────────────

    pub async fn search(&self, query: &str, limit: usize) -> Result<serde_json::Value> {
        let url = format!("{}?q={}&limit={limit}", self.url("/search"), urlencoded(query));
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    // ── Attachments ──────────────────────────────────────────────────

    pub async fn attach_file(
        &self,
        data: &[u8],
        filename: &str,
        content_type: &str,
    ) -> Result<serde_json::Value> {
        let part = reqwest::multipart::Part::bytes(data.to_vec())
            .file_name(filename.to_string())
            .mime_str(content_type)?;
        let form = reqwest::multipart::Form::new().part("file", part);
        // The doc ID in the URL is unused by the server, use a placeholder.
        let resp = self
            .client
            .post(self.url("/docs/00000000000000000000000000000000/attachments"))
            .multipart(form)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn download_attachment(&self, cid_hex: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get(self.url(&format!("/attachments/{cid_hex}")))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn get_attachment_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>> {
        let resp = self
            .client
            .get(self.url(&format!("/attachments/{cid_hex}")))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        // The manifest endpoint returns raw bytes; try to parse as JSON.
        let bytes = resp.error_for_status()?.bytes().await?;
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    // ── Graph ────────────────────────────────────────────────────────

    pub async fn add_entity(
        &self,
        kind: &str,
        props: serde_json::Value,
        visibility: Option<&str>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url("/entities"))
            .json(&serde_json::json!({
                "kind": kind,
                "props": props,
                "visibility": visibility,
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn add_edge(
        &self,
        source_id: &str,
        relation: &str,
        target_id: &str,
        weight: Option<f32>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url(&format!("/entities/{source_id}/edges")))
            .json(&serde_json::json!({
                "relation": relation,
                "target": target_id,
                "weight": weight,
                "props": {},
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn traverse(
        &self,
        entity_id: &str,
        relation: Option<&str>,
        max_depth: usize,
    ) -> Result<serde_json::Value> {
        let mut url = format!(
            "{}?max_depth={max_depth}",
            self.url(&format!("/entities/{entity_id}/traverse"))
        );
        if let Some(rel) = relation {
            url.push_str(&format!("&relation={}", urlencoded(rel)));
        }
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    // ── Audit ────────────────────────────────────────────────────────

    pub async fn retract(&self, cid_hex: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .delete(self.url(&format!("/docs/{cid_hex}")))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    // ── Admin ────────────────────────────────────────────────────────

    pub async fn status(&self) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url("/admin/status"))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
}

fn urlencoded(s: &str) -> String {
    // Minimal percent-encoding for query parameters.
    s.replace('%', "%25")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('+', "%2B")
        .replace(' ', "%20")
}
