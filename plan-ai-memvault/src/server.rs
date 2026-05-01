use std::collections::BTreeMap;
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use memvault_api::{LocalClient, MemvaultClient};
use memvault_core::{DocId, EdgeId, EntityId, Visibility};
use memvault_doc::{Document, Edge, Entity};

use crate::types::*;

#[derive(Clone)]
pub struct MemvaultServer {
    client: Arc<LocalClient>,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

impl MemvaultServer {
    pub fn new(client: Arc<LocalClient>) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for MemvaultServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Memvault MCP server — store, retrieve, search, and link memories in a \
                 local-first p2p knowledge base. Use memvault_put to store documents, \
                 memvault_search to find them, and the graph tools to build a knowledge graph."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

fn parse_visibility(vis: Option<&str>) -> Visibility {
    match vis {
        Some("federated") => Visibility::Federated,
        Some("public") => Visibility::Public,
        _ => Visibility::Internal,
    }
}

fn parse_tags(tags: &[String]) -> Vec<(String, String)> {
    tags.iter()
        .filter_map(|t| {
            let mut parts = t.splitn(2, ':');
            let scope = parts.next()?.to_string();
            let label = parts.next().unwrap_or("").to_string();
            Some((scope, label))
        })
        .collect()
}

fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>, String> {
    hex::decode(hex_str).map_err(|e| format!("invalid hex: {e}"))
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

fn entity_id_from_hex(hex_str: &str) -> Result<EntityId, String> {
    let bytes = hex_to_bytes(hex_str)?;
    if bytes.len() != 32 {
        return Err(format!("entity ID must be 32 bytes, got {}", bytes.len()));
    }
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&bytes);
    Ok(EntityId(buf))
}

fn doc_id_from_hex(hex_str: &str) -> Result<DocId, String> {
    let bytes = hex_to_bytes(hex_str)?;
    if bytes.len() != 32 {
        return Err(format!("doc ID must be 32 bytes, got {}", bytes.len()));
    }
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&bytes);
    Ok(DocId(buf))
}

#[tool_router]
impl MemvaultServer {
    #[tool(
        name = "memvault_put",
        description = "Store a memory (document with optional title and tags). Returns the hex-encoded CID of the stored document."
    )]
    async fn put(&self, Parameters(params): Parameters<PutParams>) -> String {
        let doc_id = DocId::random();
        let vis = parse_visibility(params.visibility.as_deref());
        let tags = parse_tags(&params.tags);

        let mut frontmatter = BTreeMap::new();
        if let Some(title) = &params.title {
            frontmatter.insert(
                "title".to_string(),
                serde_json::Value::String(title.clone()),
            );
        }

        let doc = Document::new(doc_id, params.text, frontmatter);

        match self.client.put_doc(doc, tags, vis).await {
            Ok(cid) => {
                serde_json::json!({
                    "cid": bytes_to_hex(&cid),
                    "status": "stored"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_get",
        description = "Retrieve a memory by its hex-encoded CID. Returns the document text and metadata."
    )]
    async fn get(&self, Parameters(params): Parameters<GetParams>) -> String {
        let doc_id = match doc_id_from_hex(&params.cid) {
            Ok(id) => id,
            Err(_) => {
                return format!(
                    "error: CID {} does not map to a 32-byte doc ID. Use the doc_id from list/search results.",
                    params.cid
                );
            }
        };

        match self.client.get_doc(&doc_id).await {
            Ok(Some(doc)) => {
                let title = doc
                    .frontmatter
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                serde_json::json!({
                    "doc_id": bytes_to_hex(&doc_id.0),
                    "title": title,
                    "body": doc.body,
                })
                .to_string()
            }
            Ok(None) => format!("error: document not found for cid {}", params.cid),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_search",
        description = "Search memories by text query. Returns matching documents with relevance scores and snippets."
    )]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> String {
        let limit = params.limit.unwrap_or(10);

        match self.client.search(&params.query, limit).await {
            Ok(hits) => {
                let results: Vec<serde_json::Value> = hits
                    .iter()
                    .map(|h| {
                        serde_json::json!({
                            "doc_id": bytes_to_hex(&h.doc_id.0),
                            "score": h.score,
                            "snippet": h.snippet,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "count": results.len(),
                    "results": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_list",
        description = "List recent memories. Optionally filter by tag scope and label."
    )]
    async fn list(&self, Parameters(params): Parameters<ListParams>) -> String {
        let limit = params.limit.unwrap_or(20);
        let tag_filter = match (params.tag_scope, params.tag_label) {
            (Some(scope), Some(label)) => Some((scope, label)),
            _ => None,
        };

        match self.client.list_docs(tag_filter, limit).await {
            Ok(docs) => {
                let results: Vec<serde_json::Value> = docs
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "doc_id": bytes_to_hex(&d.id.0),
                            "cid": bytes_to_hex(&d.cid),
                            "title": d.title,
                            "tags": d.tags.iter().map(|(s, l)| format!("{s}:{l}")).collect::<Vec<_>>(),
                            "attachment_count": d.attachment_count,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "count": results.len(),
                    "documents": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_attach",
        description = "Attach a file to memvault. Content must be base64-encoded. Returns the manifest CID. Files are now stored as standalone first-class objects."
    )]
    async fn attach(&self, Parameters(params): Parameters<AttachParams>) -> String {
        let data = match base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &params.content_base64,
        ) {
            Ok(d) => d,
            Err(e) => return format!("error: invalid base64: {e}"),
        };

        let mime_type = params
            .content_type
            .as_deref()
            .unwrap_or("application/octet-stream");
        let tags = parse_tags(&params.tags.unwrap_or_default());
        let visibility = params.visibility.as_deref().unwrap_or("internal");

        match self
            .client
            .attach_file(&data, Some(&params.filename), mime_type, tags, visibility)
            .await
        {
            Ok(cid) => {
                serde_json::json!({
                    "manifest_cid": bytes_to_hex(&cid),
                    "filename": params.filename,
                    "status": "attached"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_read_range",
        description = "Read a byte range from an attachment. Returns base64-encoded bytes for the range [start, end)."
    )]
    async fn read_range(&self, Parameters(params): Parameters<ReadRangeParams>) -> String {
        let cid = match hex_to_bytes(&params.manifest_cid) {
            Ok(b) => b,
            Err(e) => return format!("error: {e}"),
        };

        match self.client.read_attachment_range(&cid, params.start, params.end).await {
            Ok(data) => {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                serde_json::json!({
                    "data_base64": encoded,
                    "size": data.len(),
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_pin",
        description = "Pin an attachment to prevent garbage collection."
    )]
    async fn pin(&self, Parameters(params): Parameters<PinParams>) -> String {
        let cid = match hex_to_bytes(&params.manifest_cid) {
            Ok(b) => b,
            Err(e) => return format!("error: {e}"),
        };

        match self.client.pin_attachment(&cid).await {
            Ok(()) => {
                serde_json::json!({
                    "manifest_cid": params.manifest_cid,
                    "status": "pinned"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_unpin",
        description = "Unpin an attachment, allowing garbage collection."
    )]
    async fn unpin(&self, Parameters(params): Parameters<UnpinParams>) -> String {
        let cid = match hex_to_bytes(&params.manifest_cid) {
            Ok(b) => b,
            Err(e) => return format!("error: {e}"),
        };

        match self.client.unpin_attachment(&cid).await {
            Ok(()) => {
                serde_json::json!({
                    "manifest_cid": params.manifest_cid,
                    "status": "unpinned"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_extract_text",
        description = "Extract text content from an attachment (supports PDF, DOCX, HTML, Markdown, plain text)."
    )]
    async fn extract_text(&self, Parameters(params): Parameters<ExtractTextParams>) -> String {
        let cid = match hex_to_bytes(&params.manifest_cid) {
            Ok(b) => b,
            Err(e) => return format!("error: {e}"),
        };

        match self.client.read_extracted_text(&cid).await {
            Ok(Some(text)) => {
                serde_json::json!({
                    "manifest_cid": params.manifest_cid,
                    "text": text,
                })
                .to_string()
            }
            Ok(None) => {
                serde_json::json!({
                    "manifest_cid": params.manifest_cid,
                    "text": null,
                    "note": "extraction not supported for this file type"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_attachment_info",
        description = "Get the manifest metadata for an attachment (filename, MIME type, size, layout, etc.)."
    )]
    async fn attachment_info(&self, Parameters(params): Parameters<AttachmentInfoParams>) -> String {
        let cid = match hex_to_bytes(&params.manifest_cid) {
            Ok(b) => b,
            Err(e) => return format!("error: {e}"),
        };

        match self.client.get_attachment_manifest(&cid).await {
            Ok(Some(json_bytes)) => {
                // The manifest is stored as JSON, just return it
                match String::from_utf8(json_bytes) {
                    Ok(json_str) => json_str,
                    Err(_) => "error: manifest is not valid UTF-8".to_string(),
                }
            }
            Ok(None) => format!("error: manifest not found for cid {}", params.manifest_cid),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_add",
        description = "Add an entity to the knowledge graph. Returns the hex-encoded entity ID."
    )]
    async fn graph_add(&self, Parameters(params): Parameters<GraphAddParams>) -> String {
        let vis = parse_visibility(params.visibility.as_deref());
        let entity_id = EntityId::random();

        let props: BTreeMap<String, serde_json::Value> = params
            .props
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();

        let entity = Entity {
            id: entity_id.clone(),
            kind: params.kind,
            props,
            edges_out: Vec::new(),
        };

        match self.client.add_entity(entity, vis).await {
            Ok(id) => {
                serde_json::json!({
                    "entity_id": bytes_to_hex(&id.0),
                    "status": "created"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_link",
        description = "Add a directed edge between two entities in the knowledge graph. Returns the edge ID."
    )]
    async fn graph_link(&self, Parameters(params): Parameters<GraphLinkParams>) -> String {
        let source_id = match entity_id_from_hex(&params.source_id) {
            Ok(id) => id,
            Err(e) => return format!("error: {e}"),
        };
        let target_id = match entity_id_from_hex(&params.target_id) {
            Ok(id) => id,
            Err(e) => return format!("error: {e}"),
        };

        let edge_id = EdgeId::random();
        let edge = Edge {
            id: edge_id.clone(),
            relation: params.relation,
            target: target_id,
            weight: params.weight,
            props: BTreeMap::new(),
            provenance: None,
        };

        match self
            .client
            .add_edge(&source_id, edge, Visibility::Internal)
            .await
        {
            Ok(id) => {
                serde_json::json!({
                    "edge_id": bytes_to_hex(&id.0),
                    "status": "linked"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_query",
        description = "Traverse the knowledge graph from a given entity. Returns connected entities up to max_depth."
    )]
    async fn graph_query(&self, Parameters(params): Parameters<GraphQueryParams>) -> String {
        let from_id = match entity_id_from_hex(&params.from_id) {
            Ok(id) => id,
            Err(e) => return format!("error: {e}"),
        };

        let max_depth = params.max_depth.unwrap_or(2);

        match self
            .client
            .traverse(&from_id, params.relation.as_deref(), max_depth)
            .await
        {
            Ok(hits) => {
                let results: Vec<serde_json::Value> = hits
                    .iter()
                    .map(|h| {
                        let path: Vec<serde_json::Value> = h
                            .path
                            .iter()
                            .map(|(edge_id, rel)| {
                                serde_json::json!({
                                    "edge_id": bytes_to_hex(&edge_id.0),
                                    "relation": rel,
                                })
                            })
                            .collect();
                        serde_json::json!({
                            "entity_id": bytes_to_hex(&h.entity_id.0),
                            "depth": h.depth,
                            "path": path,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "count": results.len(),
                    "hits": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_retract",
        description = "Retract (soft-delete) a memory by its CID. Creates a tombstone. Returns the tombstone CID."
    )]
    async fn retract(&self, Parameters(params): Parameters<RetractParams>) -> String {
        let cid_bytes = match hex_to_bytes(&params.cid) {
            Ok(b) => b,
            Err(e) => return format!("error: {e}"),
        };

        match self.client.retract(&cid_bytes, &params.reason).await {
            Ok(tombstone) => {
                serde_json::json!({
                    "tombstone_cid": bytes_to_hex(&tombstone),
                    "status": "retracted"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_status",
        description = "Get the status of the local memvault node (block count, doc count, peer count, uptime)."
    )]
    async fn status(&self) -> String {
        match self.client.status().await {
            Ok(s) => {
                serde_json::json!({
                    "peer_id": bytes_to_hex(&s.peer_id),
                    "cluster_id": bytes_to_hex(&s.cluster_id),
                    "block_count": s.block_count,
                    "doc_count": s.doc_count,
                    "peer_count": s.peer_count,
                    "uptime_secs": s.uptime_secs,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }
}
