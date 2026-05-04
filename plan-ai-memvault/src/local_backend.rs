//! Local backend — wraps a LocalClient for direct redb access without HTTP.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;

use memvault_api::{EventBus, LocalClient, MemvaultClient};
use memvault_core::{DocId, EdgeId, EntityId, NodeRef, Visibility};
use memvault_doc::{Document, Edge, Entity};
use memvault_query::{QuotaManager, TextIndex};
use memvault_store::MemvaultStore;

use crate::backend::Backend;

pub struct LocalBackend {
    client: LocalClient,
}

impl LocalBackend {
    pub async fn open(db_path: &std::path::Path, cluster_id: Vec<u8>) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Arc::new(MemvaultStore::open(db_path)?);
        let client = LocalClient::new(
            store,
            Arc::new(RwLock::new(TextIndex::new())),
            Arc::new(RwLock::new(QuotaManager::new(Default::default()))),
            Arc::new(EventBus::new(16)),
            vec![0u8; 32], // peer_id
            cluster_id,
        );
        // Load or rebuild the text index from disk cache.
        let cache_path = db_path.with_extension("text_index.json");
        match client.load_or_rebuild_index(&cache_path).await {
            Ok((d, e, a)) => tracing::info!("text index: {d} docs, {e} entities, {a} attachments"),
            Err(e) => tracing::warn!("failed to load text index: {e}"),
        }
        Ok(Self { client })
    }
}

fn parse_visibility(s: Option<&str>) -> Visibility {
    match s {
        Some("public") => Visibility::Public,
        Some("federated") => Visibility::Federated,
        _ => Visibility::Internal,
    }
}

fn hex_to_32(hex_str: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(hex_str)?;
    if bytes.len() != 32 {
        anyhow::bail!("expected 32 bytes, got {}", bytes.len());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

#[async_trait]
impl Backend for LocalBackend {
    async fn put_doc(
        &self, body: &str, frontmatter: serde_json::Value,
        tags: Vec<(String, String)>, visibility: Option<&str>,
    ) -> Result<serde_json::Value> {
        let fm: BTreeMap<String, serde_json::Value> = match frontmatter {
            serde_json::Value::Object(m) => m.into_iter().collect(),
            _ => BTreeMap::new(),
        };
        let doc = Document::new(DocId::random(), body.to_string(), fm);
        let doc_id = hex::encode(doc.id.0);
        let vis = parse_visibility(visibility);
        let cid = self.client.put_doc(doc, tags, vis).await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "cid": hex::encode(&cid), "id": doc_id }))
    }

    async fn get_doc(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let arr = hex_to_32(id)?;
        let doc_id = DocId(arr);
        let doc = self.client.get_doc(&doc_id).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(doc.map(|d| serde_json::json!({
            "id": hex::encode(d.id.0),
            "body": d.body,
            "frontmatter": d.frontmatter,
        })))
    }

    async fn search(&self, query: &str, limit: usize, _tag_filter: Option<&str>) -> Result<serde_json::Value> {
        // Tag filtering is applied at the index level. For now, use the basic search.
        // TODO: wire tag_filter through to SearchQuery when the MemvaultClient trait supports it.
        let hits = self.client.search(query, limit).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(hits.iter().map(|h| serde_json::json!({
            "doc_id": hex::encode(h.doc_id.0),
            "score": h.score,
            "snippet": h.snippet,
        })).collect::<Vec<_>>()))
    }

    async fn list_docs(&self, tag_ns: Option<&str>, tag_val: Option<&str>, limit: usize) -> Result<serde_json::Value> {
        let tag_filter = match (tag_ns, tag_val) {
            (Some(ns), Some(val)) => Some((ns.to_string(), val.to_string())),
            _ => None,
        };
        let docs = self.client.list_docs(tag_filter, limit).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(docs.iter().map(|d| serde_json::json!({
            "id": hex::encode(d.id.0),
            "title": d.title,
            "updated_ns": d.updated_ns,
        })).collect::<Vec<_>>()))
    }

    async fn attach_file(&self, data: &[u8], filename: &str, content_type: &str) -> Result<serde_json::Value> {
        let cid = self.client.attach_file(data, Some(filename), content_type, vec![], "internal").await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "cid": hex::encode(&cid) }))
    }

    async fn download_attachment(&self, cid_hex: &str) -> Result<Vec<u8>> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.read_attachment(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn read_attachment_range(&self, cid_hex: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.read_attachment_range(&cid_bytes, start, end).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn extract_text(&self, cid_hex: &str) -> Result<Option<String>> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.read_extracted_text(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn get_attachment_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>> {
        let cid_bytes = hex::decode(cid_hex)?;
        let data = self.client.get_attachment_manifest(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        match data {
            Some(bytes) => {
                let val = serde_json::from_slice(&bytes)
                    .map_err(|e| anyhow::anyhow!("corrupt manifest JSON: {e}"))?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    async fn pin_attachment(&self, cid_hex: &str) -> Result<()> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.pin_attachment(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn unpin_attachment(&self, cid_hex: &str) -> Result<()> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.unpin_attachment(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn add_entity(&self, kind: &str, props: serde_json::Value, visibility: Option<&str>) -> Result<serde_json::Value> {
        let props_map: BTreeMap<String, serde_json::Value> = match props {
            serde_json::Value::Object(m) => m.into_iter().collect(),
            _ => BTreeMap::new(),
        };
        let entity = Entity {
            id: EntityId::random(),
            kind: kind.to_string(),
            props: props_map,
            edges_out: vec![],
        };
        let vis = parse_visibility(visibility);
        let id = self.client.add_entity(entity, vis).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "id": hex::encode(id.0) }))
    }

    async fn add_link(&self, source: &str, target: &str, relation: &str, weight: Option<f32>) -> Result<serde_json::Value> {
        let source_ref = NodeRef::from_tag_label(source)
            .ok_or_else(|| anyhow::anyhow!("invalid source: {source}"))?;
        let target_ref = NodeRef::from_tag_label(target)
            .ok_or_else(|| anyhow::anyhow!("invalid target: {target}"))?;
        let edge = Edge {
            id: EdgeId::random(),
            relation: relation.to_string(),
            target: target_ref,
            weight,
            props: BTreeMap::new(),
            provenance: None,
        };
        let vis = Visibility::Internal;
        let edge_id = self.client.add_link(&source_ref, edge, vis).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "edge_id": hex::encode(edge_id.0) }))
    }

    async fn edges_of(&self, node: &str) -> Result<serde_json::Value> {
        let node_ref = NodeRef::from_tag_label(node)
            .ok_or_else(|| anyhow::anyhow!("invalid node: {node}"))?;
        let edges = self.client.edges_of(&node_ref).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(edges.iter().map(|(src, edge)| serde_json::json!({
            "edge_id": hex::encode(edge.id.0),
            "source": src.tag_label(),
            "target": edge.target.tag_label(),
            "relation": edge.relation,
            "weight": edge.weight,
        })).collect::<Vec<_>>()))
    }

    async fn delete_link(&self, edge_id: &str) -> Result<serde_json::Value> {
        let bytes = hex::decode(edge_id)?;
        self.client.retract(&bytes, "deleted via MCP").await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "status": "removed" }))
    }

    async fn retract(&self, cid_hex: &str, reason: &str) -> Result<serde_json::Value> {
        let cid_bytes = hex::decode(cid_hex)?;
        let tombstone = self.client.retract(&cid_bytes, reason).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "cid": hex::encode(&tombstone) }))
    }

    async fn status(&self) -> Result<serde_json::Value> {
        let s = self.client.status().await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(s))
    }
}
