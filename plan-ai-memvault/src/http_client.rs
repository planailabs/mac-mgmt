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
        let resp = self
            .client
            .get(self.url(&format!("/docs/{id}")))
            .send()
            .await?;
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
        let url = format!(
            "{}?q={}&limit={limit}",
            self.url("/search"),
            urlencoded(query)
        );
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    // ── Files ────────────────────────────────────────────────────────

    pub async fn upload_file(
        &self,
        data: &[u8],
        filename: &str,
        content_type: &str,
    ) -> Result<serde_json::Value> {
        let part = reqwest::multipart::Part::bytes(data.to_vec())
            .file_name(filename.to_string())
            .mime_str(content_type)?;
        let form = reqwest::multipart::Form::new().part("file", part);
        let resp = self
            .client
            .post(self.url("/files"))
            .multipart(form)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    pub async fn download_file(&self, cid_hex: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get(self.url(&format!("/files/{cid_hex}")))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn get_file_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>> {
        let resp = self
            .client
            .get(self.url(&format!("/files/{cid_hex}/manifest")))
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
        props: std::collections::BTreeMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url("/links"))
            .json(&serde_json::json!({
                "source": source,
                "target": target,
                "relation": relation,
                "weight": weight,
                "props": props,
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
    async fn put_doc(
        &self,
        body: &str,
        frontmatter: serde_json::Value,
        tags: Vec<(String, String)>,
        visibility: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.put_doc(body, frontmatter, tags, visibility).await
    }
    async fn get_doc(&self, id: &str) -> Result<Option<serde_json::Value>> {
        self.get_doc(id).await
    }
    async fn search(
        &self,
        query: &str,
        limit: usize,
        _tag_filter: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.search(query, limit).await
    }
    async fn edit_doc(&self, id: &str, patch_json: serde_json::Value) -> Result<serde_json::Value> {
        let resp = self
            .client
            .put(self.url(&format!("/docs/{id}")))
            .json(&serde_json::json!({ "patch": patch_json }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn list_docs(
        &self,
        tag_ns: Option<&str>,
        tag_val: Option<&str>,
        limit: usize,
    ) -> Result<serde_json::Value> {
        self.list_docs(tag_ns, tag_val, limit).await
    }
    async fn history_of(&self, doc_id: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url(&format!("/docs/{doc_id}/history")))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn upload_file(
        &self,
        data: &[u8],
        filename: &str,
        content_type: &str,
    ) -> Result<serde_json::Value> {
        self.upload_file(data, filename, content_type).await
    }
    async fn download_file(&self, cid_hex: &str) -> Result<Vec<u8>> {
        self.download_file(cid_hex).await
    }
    async fn read_file_range(&self, cid_hex: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        // REST API doesn't have a range endpoint yet; download full and slice.
        let data = self.download_file(cid_hex).await?;
        let s = start as usize;
        let e = (end as usize).min(data.len());
        Ok(if s < data.len() {
            data[s..e].to_vec()
        } else {
            vec![]
        })
    }
    async fn extract_text(&self, cid_hex: &str) -> Result<Option<String>> {
        let data = self.download_file(cid_hex).await?;
        let manifest = self.get_file_manifest(cid_hex).await?;
        let mime = manifest
            .and_then(|m| {
                m.get("mime_type")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());
        match std::panic::catch_unwind(move || {
            let registry = memvault_extract::ExtractionRegistry::with_defaults();
            registry.extract(&data, &mime, &memvault_extract::ExtractionHints::default())
        }) {
            Ok(Ok(extracted)) => Ok(Some(extracted.text)),
            Ok(Err(_)) | Err(_) => Ok(None),
        }
    }
    async fn get_file_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>> {
        self.get_file_manifest(cid_hex).await
    }
    async fn pin_file(&self, _cid_hex: &str) -> Result<()> {
        // Pin/unpin REST endpoints not yet available; no-op for HTTP mode.
        Ok(())
    }
    async fn unpin_file(&self, _cid_hex: &str) -> Result<()> {
        Ok(())
    }
    async fn add_entity(
        &self,
        kind: &str,
        props: serde_json::Value,
        visibility: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.add_entity(kind, props, visibility).await
    }
    async fn get_entity(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let node_id = if id.contains(':') {
            id.to_string()
        } else {
            format!("entity:{id}")
        };
        let resp = self
            .client
            .get(self.url(&format!("/nodes/{}", urlencoded(&node_id))))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(resp.error_for_status()?.json().await?))
    }
    async fn list_entities(&self, limit: usize) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url(&format!("/nodes?limit={limit}")))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn entity_history(&self, _id: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url(&format!("/audit?limit=100")))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn traverse_from(
        &self,
        from: &str,
        _relation: Option<&str>,
        _max_depth: usize,
    ) -> Result<serde_json::Value> {
        self.edges_of(from).await
    }
    async fn add_link(
        &self,
        source: &str,
        target: &str,
        relation: &str,
        weight: Option<f32>,
        props: std::collections::BTreeMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.add_link(source, target, relation, weight, props).await
    }
    async fn edges_of(&self, node: &str) -> Result<serde_json::Value> {
        self.edges_of(node).await
    }
    async fn delete_link(&self, edge_id: &str, source: &str) -> Result<serde_json::Value> {
        let url = format!(
            "{}?source={}",
            self.url(&format!("/links/{edge_id}")),
            urlencoded(source)
        );
        self.client.delete(url).send().await?.error_for_status()?;
        Ok(serde_json::json!({ "status": "removed" }))
    }
    async fn retract(&self, cid_hex: &str, _reason: &str) -> Result<serde_json::Value> {
        self.retract(cid_hex).await
    }
    async fn retract_node(&self, node_id: &str, _reason: &str) -> Result<serde_json::Value> {
        self.client
            .delete(self.url(&format!("/nodes/{}", urlencoded(node_id))))
            .send()
            .await?
            .error_for_status()?;
        Ok(serde_json::json!({ "status": "retracted" }))
    }
    async fn search_unified(&self, query: &str, limit: usize) -> Result<serde_json::Value> {
        let url = format!(
            "{}?q={}&limit={limit}",
            self.url("/search"),
            urlencoded(query)
        );
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn resolve_label(&self, _node_id: &str) -> Result<Option<String>> {
        // No REST endpoint for label resolution; return None in HTTP mode.
        Ok(None)
    }
    async fn list_all(&self, view: Option<&str>, limit: usize) -> Result<serde_json::Value> {
        let mut url = format!("{}?limit={limit}", self.url("/nodes"));
        if let Some(v) = view {
            url.push_str(&format!("&view={}", urlencoded(v)));
        }
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn add_tags(
        &self,
        node_id: &str,
        tags: Vec<(String, String)>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .put(self.url(&format!("/tags/{}", urlencoded(node_id))))
            .json(&serde_json::json!({ "tags": tags }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn remove_tags(
        &self,
        node_id: &str,
        tags: Vec<(String, String)>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .delete(self.url(&format!("/tags/{}", urlencoded(node_id))))
            .json(&serde_json::json!({ "tags": tags }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn get_tags(&self, node_id: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url(&format!("/tags/{}", urlencoded(node_id))))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn list_views(&self) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url("/views"))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn create_view(
        &self,
        name: &str,
        tags: Vec<(String, String)>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url("/views"))
            .json(&serde_json::json!({ "name": name, "tags": tags }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn update_view(
        &self,
        name: &str,
        tags: Vec<(String, String)>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .put(self.url(&format!("/views/{}", urlencoded(name))))
            .json(&serde_json::json!({ "name": name, "tags": tags }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn delete_view(&self, name: &str) -> Result<serde_json::Value> {
        self.client
            .delete(self.url(&format!("/views/{}", urlencoded(name))))
            .send()
            .await?
            .error_for_status()?;
        Ok(serde_json::json!({ "status": "deleted" }))
    }
    async fn view_members(&self, name: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url(&format!("/views/{}/members", urlencoded(name))))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn bucket_list(&self) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(self.url("/buckets"))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn bucket_create(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url("/buckets"))
            .json(&serde_json::json!({ "name": name, "description": description }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn bucket_get(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let resp = self
            .client
            .get(self.url(&format!("/buckets/{id}")))
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(resp.error_for_status()?.json().await?))
    }

    async fn bucket_rename(&self, id: &str, new_name: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .patch(self.url(&format!("/buckets/{id}")))
            .json(&serde_json::json!({ "name": new_name }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn bucket_attach(&self, id: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url(&format!("/buckets/{id}/attach")))
            .json(&serde_json::json!({}))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn bucket_archive(&self, id: &str, reason: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .post(self.url(&format!("/buckets/{id}/archive")))
            .json(&serde_json::json!({ "reason": reason }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn audit(&self, limit: usize, op_kind: Option<&str>) -> Result<serde_json::Value> {
        let mut url = format!("{}?limit={limit}", self.url("/audit"));
        if let Some(k) = op_kind {
            url.push_str(&format!("&kind={}", urlencoded(k)));
        }
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }
    async fn status(&self) -> Result<serde_json::Value> {
        self.status().await
    }
}

fn urlencoded(s: &str) -> String {
    // Percent-encode query parameter values.
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => write!(out, "%{b:02X}").unwrap(),
        }
    }
    out
}
