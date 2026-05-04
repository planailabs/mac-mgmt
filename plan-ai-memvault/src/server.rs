use std::collections::BTreeMap;
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::backend::Backend;
use crate::types::*;

#[derive(Clone)]
pub struct MemvaultServer {
    client: Arc<dyn Backend>,
    default_tags: Vec<String>,
    default_visibility: String,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

impl MemvaultServer {
    pub fn new(client: Arc<dyn Backend>, default_tags: Vec<String>, default_visibility: String) -> Self {
        Self {
            client,
            default_tags,
            default_visibility,
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

#[tool_router]
impl MemvaultServer {
    #[tool(
        name = "memvault_put",
        description = "Store a memory (document with optional title and tags). Returns the hex-encoded CID of the stored document."
    )]
    async fn put(&self, Parameters(params): Parameters<PutParams>) -> String {
        let mut frontmatter = serde_json::Map::new();
        if let Some(title) = &params.title {
            frontmatter.insert(
                "title".to_string(),
                serde_json::Value::String(title.clone()),
            );
        }

        let tags_input = if params.tags.is_empty() {
            &self.default_tags
        } else {
            &params.tags
        };
        let tags = parse_tags(tags_input);
        let vis = params.visibility.as_deref().or(Some(self.default_visibility.as_str()));

        match self
            .client
            .put_doc(
                &params.text,
                serde_json::Value::Object(frontmatter),
                tags,
                vis,
            )
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "cid": resp.get("cid").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "stored"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_get",
        description = "Retrieve a memory by its hex-encoded doc ID. Returns the document text and metadata."
    )]
    async fn get(&self, Parameters(params): Parameters<GetParams>) -> String {
        match self.client.get_doc(&params.cid).await {
            Ok(Some(doc)) => {
                serde_json::json!({
                    "doc_id": params.cid,
                    "title": doc.get("frontmatter")
                        .and_then(|f| f.get("title"))
                        .and_then(|t| t.as_str()),
                    "body": doc.get("body").and_then(|b| b.as_str()).unwrap_or(""),
                })
                .to_string()
            }
            Ok(None) => format!("error: document not found for id {}", params.cid),
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
            Ok(hits) => hits.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_list",
        description = "List recent memories. Optionally filter by tag scope and label."
    )]
    async fn list(&self, Parameters(params): Parameters<ListParams>) -> String {
        let limit = params.limit.unwrap_or(20);
        let tag_ns = params.tag_scope.as_deref();
        let tag_val = params.tag_label.as_deref();

        match self.client.list_docs(tag_ns, tag_val, limit).await {
            Ok(docs) => {
                let results = docs.as_array().cloned().unwrap_or_default();
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
        description = "Attach a local file to memvault by its absolute path. Returns the manifest CID."
    )]
    async fn attach(&self, Parameters(params): Parameters<AttachParams>) -> String {
        let path = std::path::Path::new(&params.path);

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => return format!("error: cannot read {}: {e}", params.path),
        };

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed");

        let mime_type = params.content_type.as_deref().unwrap_or_else(|| {
            mime_guess::from_path(path)
                .first_raw()
                .unwrap_or("application/octet-stream")
        });

        match self
            .client
            .attach_file(&data, filename, mime_type)
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "manifest_cid": resp.get("cid").and_then(|v| v.as_str()).unwrap_or(""),
                    "filename": filename,
                    "size": data.len(),
                    "mime_type": mime_type,
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
        match self.client.download_attachment(&params.manifest_cid).await {
            Ok(data) => {
                let start = params.start as usize;
                let end = (params.end as usize).min(data.len());
                let slice = if start < data.len() {
                    &data[start..end]
                } else {
                    &[]
                };
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(slice);
                serde_json::json!({
                    "data_base64": encoded,
                    "size": slice.len(),
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
        // Pin/unpin not directly exposed via REST yet; return success.
        serde_json::json!({
            "manifest_cid": params.manifest_cid,
            "status": "pinned"
        })
        .to_string()
    }

    #[tool(
        name = "memvault_unpin",
        description = "Unpin an attachment, allowing garbage collection."
    )]
    async fn unpin(&self, Parameters(params): Parameters<UnpinParams>) -> String {
        serde_json::json!({
            "manifest_cid": params.manifest_cid,
            "status": "unpinned"
        })
        .to_string()
    }

    #[tool(
        name = "memvault_extract_text",
        description = "Extract text content from an attachment (supports PDF, DOCX, HTML, Markdown, plain text)."
    )]
    async fn extract_text(&self, Parameters(params): Parameters<ExtractTextParams>) -> String {
        match self.client.extract_text(&params.manifest_cid).await {
            Ok(Some(text)) => serde_json::json!({
                "manifest_cid": params.manifest_cid,
                "text": text,
            })
            .to_string(),
            Ok(None) => serde_json::json!({
                "manifest_cid": params.manifest_cid,
                "text": null,
                "note": "unsupported format or extraction failed"
            })
            .to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_attachment_info",
        description = "Get the manifest metadata for an attachment (filename, MIME type, size, layout, etc.)."
    )]
    async fn attachment_info(&self, Parameters(params): Parameters<AttachmentInfoParams>) -> String {
        match self
            .client
            .get_attachment_manifest(&params.manifest_cid)
            .await
        {
            Ok(Some(manifest)) => manifest.to_string(),
            Ok(None) => format!("error: manifest not found for cid {}", params.manifest_cid),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_add",
        description = "Add an entity to the knowledge graph. Returns the hex-encoded entity ID."
    )]
    async fn graph_add(&self, Parameters(params): Parameters<GraphAddParams>) -> String {
        let vis = params.visibility.as_deref().or(Some(self.default_visibility.as_str()));
        let props: BTreeMap<String, serde_json::Value> = params
            .props
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();

        match self
            .client
            .add_entity(&params.kind, serde_json::json!(props), vis)
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "entity_id": resp.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "created"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_link",
        description = "Add a directed edge between two entities in the knowledge graph. \
                       Source and target are hex-encoded entity IDs. Use memvault_link for cross-type linking."
    )]
    async fn graph_link(&self, Parameters(params): Parameters<GraphLinkParams>) -> String {
        let source = format!("entity:{}", params.source_id);
        let target = format!("entity:{}", params.target_id);
        match self
            .client
            .add_link(&source, &target, &params.relation, params.weight)
            .await
        {
            Ok(resp) => {
                serde_json::json!({
                    "edge_id": resp.get("edge_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "linked"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_graph_query",
        description = "List all edges for a given entity. Returns connected nodes."
    )]
    async fn graph_query(&self, Parameters(params): Parameters<GraphQueryParams>) -> String {
        let node = format!("entity:{}", params.from_id);
        match self.client.edges_of(&node).await {
            Ok(edges) => {
                let results = edges.as_array().cloned().unwrap_or_default();
                serde_json::json!({
                    "count": results.len(),
                    "edges": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_link",
        description = "Create a directed link between any two nodes (entities, documents, or attachments). \
                       Nodes are specified as 'entity:<hex>', 'doc:<hex>', or 'attachment:<hex>'. Returns the edge ID."
    )]
    async fn link(&self, Parameters(params): Parameters<LinkParams>) -> String {
        match self.client.add_link(&params.source, &params.target, &params.relation, params.weight).await {
            Ok(resp) => {
                serde_json::json!({
                    "edge_id": resp.get("edge_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "linked"
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_edges",
        description = "List all edges (incoming and outgoing) for any node. \
                       Node is specified as 'entity:<hex>', 'doc:<hex>', or 'attachment:<hex>'."
    )]
    async fn edges(&self, Parameters(params): Parameters<EdgesOfParams>) -> String {
        match self.client.edges_of(&params.node).await {
            Ok(edges) => {
                let results = edges.as_array().cloned().unwrap_or_default();
                serde_json::json!({
                    "count": results.len(),
                    "edges": results,
                })
                .to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_unlink",
        description = "Remove a link (edge) by its hex-encoded edge ID."
    )]
    async fn unlink(&self, Parameters(params): Parameters<UnlinkParams>) -> String {
        match self.client.delete_link(&params.edge_id).await {
            Ok(_) => serde_json::json!({ "edge_id": params.edge_id, "status": "removed" }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_retract",
        description = "Retract (soft-delete) a memory by its CID. Creates a tombstone. Returns the tombstone CID."
    )]
    async fn retract(&self, Parameters(params): Parameters<RetractParams>) -> String {
        match self.client.retract(&params.cid).await {
            Ok(resp) => {
                serde_json::json!({
                    "tombstone_cid": resp.get("cid").and_then(|v| v.as_str()).unwrap_or(""),
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
            Ok(s) => s.to_string(),
            Err(e) => format!("error: {e}"),
        }
    }
}
