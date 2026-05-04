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

    async fn edit_doc(&self, id: &str, patch_json: serde_json::Value) -> Result<serde_json::Value> {
        let arr = hex_to_32(id)?;
        let doc_id = DocId(arr);
        // Parse patch from JSON: { "ops": [{"Retain": n}, {"Insert": "text"}, {"Delete": n}] }
        let patch: memvault_doc::TextPatch = serde_json::from_value(patch_json)?;
        let cid = self.client.edit_doc(&doc_id, patch).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "cid": hex::encode(&cid) }))
    }

    async fn history_of(&self, doc_id: &str) -> Result<serde_json::Value> {
        let arr = hex_to_32(doc_id)?;
        let did = DocId(arr);
        let records = self.client.history_of(&did).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(records.iter().map(|r| serde_json::json!({
            "cid": hex::encode(&r.cid),
            "op_kind": format!("{:?}", r.op_kind),
            "wall_ns": r.wall_ns,
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

    async fn upload_file(&self, data: &[u8], filename: &str, content_type: &str) -> Result<serde_json::Value> {
        let cid = self.client.upload_file(data, Some(filename), content_type, vec![], "internal").await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "cid": hex::encode(&cid) }))
    }

    async fn download_file(&self, cid_hex: &str) -> Result<Vec<u8>> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.read_file(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn read_file_range(&self, cid_hex: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.read_file_range(&cid_bytes, start, end).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn extract_text(&self, cid_hex: &str) -> Result<Option<String>> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.read_extracted_text(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn get_file_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>> {
        let cid_bytes = hex::decode(cid_hex)?;
        let data = self.client.get_file_manifest(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        match data {
            Some(bytes) => {
                let val = serde_json::from_slice(&bytes)
                    .map_err(|e| anyhow::anyhow!("corrupt manifest JSON: {e}"))?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    async fn pin_file(&self, cid_hex: &str) -> Result<()> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.pin_file(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn unpin_file(&self, cid_hex: &str) -> Result<()> {
        let cid_bytes = hex::decode(cid_hex)?;
        self.client.unpin_file(&cid_bytes).await.map_err(|e| anyhow::anyhow!("{e}"))
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

    async fn get_entity(&self, id: &str) -> Result<Option<serde_json::Value>> {
        let arr = hex_to_32(id)?;
        let eid = EntityId(arr);
        let entity = self.client.get_entity(&eid).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(entity.map(|e| serde_json::json!({
            "id": hex::encode(e.id.0),
            "kind": e.kind,
            "props": e.props,
            "edges": e.edges_out.iter().map(|edge| serde_json::json!({
                "id": hex::encode(edge.id.0),
                "relation": edge.relation,
                "target": edge.target.tag_label(),
                "weight": edge.weight,
                "props": edge.props,
            })).collect::<Vec<_>>(),
        })))
    }

    async fn list_entities(&self, limit: usize) -> Result<serde_json::Value> {
        let entities = self.client.list_entities(limit).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(entities.iter().map(|e| serde_json::json!({
            "id": hex::encode(e.id.0),
            "kind": e.kind,
            "label": e.props.get("name").or_else(|| e.props.get("title")).and_then(|v| v.as_str()).unwrap_or(&e.kind),
            "edge_count": e.edges_out.len(),
        })).collect::<Vec<_>>()))
    }

    async fn entity_history(&self, id: &str) -> Result<serde_json::Value> {
        let arr = hex_to_32(id)?;
        let eid = EntityId(arr);
        let records = self.client.entity_history(&eid).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(records.iter().map(|r| serde_json::json!({
            "cid": hex::encode(&r.cid),
            "op_kind": format!("{:?}", r.op_kind),
            "wall_ns": r.wall_ns,
        })).collect::<Vec<_>>()))
    }

    async fn traverse_from(&self, from: &str, relation: Option<&str>, max_depth: usize) -> Result<serde_json::Value> {
        let node_ref = NodeRef::from_tag_label(from)
            .ok_or_else(|| anyhow::anyhow!("invalid node: {from}"))?;
        let hits = self.client.traverse_from(&node_ref, relation, max_depth).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(hits.iter().map(|h| serde_json::json!({
            "node": h.node.tag_label(),
            "depth": h.depth,
            "path": h.path.iter().map(|(eid, rel)| serde_json::json!({ "edge_id": hex::encode(eid.0), "relation": rel })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>()))
    }

    async fn add_link(&self, source: &str, target: &str, relation: &str, weight: Option<f32>, props: BTreeMap<String, serde_json::Value>) -> Result<serde_json::Value> {
        let source_ref = NodeRef::from_tag_label(source)
            .ok_or_else(|| anyhow::anyhow!("invalid source: {source}"))?;
        let target_ref = NodeRef::from_tag_label(target)
            .ok_or_else(|| anyhow::anyhow!("invalid target: {target}"))?;
        let edge = Edge {
            id: EdgeId::random(),
            relation: relation.to_string(),
            target: target_ref,
            weight,
            props,
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
            "props": edge.props,
        })).collect::<Vec<_>>()))
    }

    async fn delete_link(&self, edge_id: &str, source: &str) -> Result<serde_json::Value> {
        let bytes = hex::decode(edge_id)?;
        if bytes.len() != 32 {
            anyhow::bail!("edge ID must be 32 bytes");
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        let eid = EdgeId(arr);
        let source_ref = NodeRef::from_tag_label(source)
            .ok_or_else(|| anyhow::anyhow!("invalid source: {source}"))?;
        self.client.remove_link_from(&source_ref, &eid).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "status": "removed" }))
    }

    async fn retract(&self, cid_hex: &str, reason: &str) -> Result<serde_json::Value> {
        let cid_bytes = hex::decode(cid_hex)?;
        let tombstone = self.client.retract(&cid_bytes, reason).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "cid": hex::encode(&tombstone) }))
    }

    async fn search_unified(&self, query: &str, limit: usize) -> Result<serde_json::Value> {
        let hits = self.client.search_unified(query, limit).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(hits.iter().map(|h| serde_json::json!({
            "node_id": h.node_id,
            "node_type": h.node_type,
            "label": h.label,
            "score": h.score,
            "snippet": h.snippet,
            "match_contexts": h.match_contexts,
        })).collect::<Vec<_>>()))
    }

    async fn resolve_label(&self, node_id: &str) -> Result<Option<String>> {
        self.client.resolve_label(node_id).await.map_err(|e| anyhow::anyhow!("{e}"))
    }

    async fn list_all(&self, view: Option<&str>, limit: usize) -> Result<serde_json::Value> {
        let items = self.client.list_all(view, limit).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(items.iter().map(|(id, nt, label, tags)| serde_json::json!({
            "node_id": id,
            "node_type": nt,
            "label": label,
            "tags": tags,
        })).collect::<Vec<_>>()))
    }

    async fn add_tags(&self, node_id: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value> {
        self.client.add_tags(node_id, tags).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "status": "tags_added" }))
    }
    async fn remove_tags(&self, node_id: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value> {
        self.client.remove_tags(node_id, tags).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "status": "tags_removed" }))
    }
    async fn get_tags(&self, node_id: &str) -> Result<serde_json::Value> {
        let tags = self.client.get_tags(node_id).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "tags": tags }))
    }

    async fn list_views(&self) -> Result<serde_json::Value> {
        let views = self.client.list_views().await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(views))
    }

    async fn create_view(&self, name: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value> {
        let view = memvault_api::View {
            name: name.to_string(),
            tags,
            created_ns: memvault_core::wall_ns(), cid: String::new(),
        };
        self.client.create_view(view).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "name": name, "status": "created" }))
    }

    async fn update_view(&self, name: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value> {
        let view = memvault_api::View {
            name: name.to_string(),
            tags,
            created_ns: memvault_core::wall_ns(), cid: String::new(),
        };
        self.client.update_view(view).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "name": name, "status": "updated" }))
    }

    async fn delete_view(&self, name: &str) -> Result<serde_json::Value> {
        self.client.delete_view(name).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "name": name, "status": "deleted" }))
    }

    async fn retract_node(&self, node_id: &str, reason: &str) -> Result<serde_json::Value> {
        self.client.retract_node(node_id, reason).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "node_id": node_id, "status": "retracted" }))
    }

    async fn view_members(&self, name: &str) -> Result<serde_json::Value> {
        let members = self.client.view_members(name).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!({ "view": name, "count": members.len(), "members": members }))
    }

    async fn audit(&self, limit: usize, op_kind: Option<&str>) -> Result<serde_json::Value> {
        use memvault_query::AuditQuery;
        let query = AuditQuery {
            op_kind: op_kind.map(|k| match k {
                "DocCreate" => memvault_query::OpKind::DocCreate,
                "DocEdit" => memvault_query::OpKind::DocEdit,
                "AttachFile" => memvault_query::OpKind::AttachFile,
                "EntityCreate" => memvault_query::OpKind::EntityCreate,
                "EdgeAdd" => memvault_query::OpKind::EdgeAdd,
                "Retract" => memvault_query::OpKind::Retract,
                other => memvault_query::OpKind::Other(other.to_string()),
            }),
            limit: Some(limit),
            ..Default::default()
        };
        let records = self.client.audit(query).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(records.iter().map(|r| serde_json::json!({
            "cid": hex::encode(&r.cid),
            "op_kind": format!("{:?}", r.op_kind),
            "author": hex::encode(&r.author),
            "wall_ns": r.wall_ns,
            "tags": r.tags,
        })).collect::<Vec<_>>()))
    }

    async fn status(&self) -> Result<serde_json::Value> {
        let s = self.client.status().await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(serde_json::json!(s))
    }
}
