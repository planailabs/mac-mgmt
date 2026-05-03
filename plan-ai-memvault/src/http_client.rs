use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, HeaderValue};

use crate::backend::Backend;

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
            .get(self.url(&format!("/attachments/{cid_hex}/manifest")))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(resp.error_for_status()?.json().await?))
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


    // ── Links (cross-type edges) ──────────────────────────────────────

    pub async fn add_link(
        &self,
        source: &str,
        target: &str,
        relation: &str,
        weight: Option<f32>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url("/links"))
            .json(&serde_json::json!({
                "source": source,
                "target": target,
                "relation": relation,
                "weight": weight,
                "props": {},
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn edges_of(&self, node: &str) -> Result<serde_json::Value> {
        let url = format!("{}?node={}", self.url("/links"), urlencoded(node));
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn delete_link(&self, edge_id: &str) -> Result<serde_json::Value> {
        self.client
            .delete(self.url(&format!("/links/{edge_id}")))
            .send()
            .await?
            .error_for_status()?;
        // 204 No Content returns empty body
        Ok(serde_json::json!({ "status": "removed" }))
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

#[async_trait]
impl Backend for HttpClient {
    async fn put_doc(&self, body: &str, frontmatter: serde_json::Value, tags: Vec<(String, String)>, visibility: Option<&str>) -> Result<serde_json::Value> {
        self.put_doc(body, frontmatter, tags, visibility).await
    }
    async fn get_doc(&self, id: &str) -> Result<Option<serde_json::Value>> { self.get_doc(id).await }
    async fn search(&self, query: &str, limit: usize) -> Result<serde_json::Value> { self.search(query, limit).await }
    async fn list_docs(&self, tag_ns: Option<&str>, tag_val: Option<&str>, limit: usize) -> Result<serde_json::Value> { self.list_docs(tag_ns, tag_val, limit).await }
    async fn attach_file(&self, data: &[u8], filename: &str, content_type: &str) -> Result<serde_json::Value> { self.attach_file(data, filename, content_type).await }
    async fn download_attachment(&self, cid_hex: &str) -> Result<Vec<u8>> { self.download_attachment(cid_hex).await }
    async fn get_attachment_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>> { self.get_attachment_manifest(cid_hex).await }
    async fn add_entity(&self, kind: &str, props: serde_json::Value, visibility: Option<&str>) -> Result<serde_json::Value> { self.add_entity(kind, props, visibility).await }
    async fn add_link(&self, source: &str, target: &str, relation: &str, weight: Option<f32>) -> Result<serde_json::Value> { self.add_link(source, target, relation, weight).await }
    async fn edges_of(&self, node: &str) -> Result<serde_json::Value> { self.edges_of(node).await }
    async fn delete_link(&self, edge_id: &str) -> Result<serde_json::Value> { self.delete_link(edge_id).await }
    async fn retract(&self, cid_hex: &str) -> Result<serde_json::Value> { self.retract(cid_hex).await }
    async fn status(&self) -> Result<serde_json::Value> { self.status().await }
}

fn urlencoded(s: &str) -> String {
    // Minimal percent-encoding for query parameters.
    s.replace('%', "%25")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('+', "%2B")
        .replace(' ', "%20")
}
