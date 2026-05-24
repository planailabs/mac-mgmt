use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::backend::Backend;
use crate::types::*;
use crate::vfs::Vfs;

/// Ensure a node ID has the `entity:` prefix, without double-prefixing.
/// Accepts both bare hex IDs and already-prefixed `entity:<hex>` IDs.
fn ensure_entity_id(id: &str) -> String {
    if let Some(hex) = id.strip_prefix("entity:") {
        format!("entity:{hex}")
    } else {
        format!("entity:{id}")
    }
}

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
                 memvault_search to find them, the graph tools to build a knowledge graph, \
                 and the VFS tools (memvault_vfs_*) to organise nodes into a virtual \
                 filesystem hierarchy with directories, paths, and tree views."
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

/// Helper: pass-through a successful JSON response, or format error.
macro_rules! ok_or_err {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v.to_string(),
            Err(e) => format!("error: {e}"),
        }
    };
}

#[tool_router]
impl MemvaultServer {
    // ── Documents ──────────────────────────────────────────────────

    #[tool(
        name = "memvault_put",
        description = "Store a memory (document with optional title and tags). Returns the hex-encoded CID and doc ID."
    )]
    async fn put(&self, Parameters(params): Parameters<PutParams>) -> String {
        let mut frontmatter = serde_json::Map::new();
        if let Some(title) = &params.title {
            frontmatter.insert("title".to_string(), serde_json::Value::String(title.clone()));
        }
        let tags_input = if params.tags.is_empty() { &self.default_tags } else { &params.tags };
        let tags = parse_tags(tags_input);
        let vis = params.visibility.as_deref().or(Some(self.default_visibility.as_str()));
        match self.client.put_doc(&params.text, serde_json::Value::Object(frontmatter), tags, vis).await {
            Ok(resp) => {
                let raw_id = resp.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let node_id = if raw_id.contains(':') { raw_id.to_string() } else { format!("doc:{raw_id}") };
                let mut result = serde_json::json!({
                    "node_id": node_id,
                    "cid": resp.get("cid").and_then(|v| v.as_str()).unwrap_or(""),
                    "status": "stored"
                });
                if let Some(vfs_path) = &params.vfs_path {
                    let vfs = Vfs::new(self.client.as_ref());
                    match vfs.link(vfs_path, &node_id).await {
                        Ok(_) => { result["vfs_path"] = serde_json::json!(vfs_path); }
                        Err(e) => { result["vfs_error"] = serde_json::json!(e.to_string()); }
                    }
                }
                result.to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_get", description = "Retrieve a memory by its hex-encoded doc ID.")]
    async fn get(&self, Parameters(params): Parameters<GetParams>) -> String {
        match self.client.get_doc(&params.cid).await {
            Ok(Some(doc)) => serde_json::json!({
                "doc_id": params.cid,
                "title": doc.get("frontmatter").and_then(|f| f.get("title")).and_then(|t| t.as_str()),
                "body": doc.get("body").and_then(|b| b.as_str()).unwrap_or(""),
            }).to_string(),
            Ok(None) => format!("error: document not found for id {}", params.cid),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_search", description = "Search memories by text query. Optionally filter by tag (scope:label format).")]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> String {
        let limit = params.limit.unwrap_or(10);
        ok_or_err!(self.client.search(&params.query, limit, params.tag_filter.as_deref()).await)
    }

    #[tool(name = "memvault_list", description = "List recent documents. Optionally filter by tag scope and label.")]
    async fn list(&self, Parameters(params): Parameters<ListParams>) -> String {
        let limit = params.limit.unwrap_or(20);
        ok_or_err!(self.client.list_docs(params.tag_scope.as_deref(), params.tag_label.as_deref(), limit).await)
    }

    #[tool(name = "memvault_doc_history", description = "View the operation history for a document by its hex-encoded ID.")]
    async fn doc_history(&self, Parameters(params): Parameters<DocHistoryParams>) -> String {
        ok_or_err!(self.client.history_of(&params.doc_id).await)
    }

    // ── Attachments ────────────────────────────────────────────────

    #[tool(name = "memvault_upload_file", description = "Upload a local file to memvault by its absolute path. Returns the manifest CID.")]
    async fn upload_file(&self, Parameters(params): Parameters<UploadFileParams>) -> String {
        let path = std::path::Path::new(&params.path);
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => return format!("error: cannot read {}: {e}", params.path),
        };
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed");
        let mime_type = params.content_type.as_deref().unwrap_or_else(|| {
            mime_guess::from_path(path).first_raw().unwrap_or("application/octet-stream")
        });
        match self.client.upload_file(&data, filename, mime_type).await {
            Ok(resp) => {
                let raw_cid = resp.get("cid").and_then(|v| v.as_str()).unwrap_or("");
                let node_id = if raw_cid.contains(':') { raw_cid.to_string() } else { format!("file:{raw_cid}") };
                let mut result = serde_json::json!({
                    "node_id": node_id, "filename": filename,
                    "size": data.len(), "mime_type": mime_type, "status": "uploaded"
                });
                if let Some(vfs_path) = &params.vfs_path {
                    let vfs = Vfs::new(self.client.as_ref());
                    match vfs.link(vfs_path, &node_id).await {
                        Ok(_) => { result["vfs_path"] = serde_json::json!(vfs_path); }
                        Err(e) => { result["vfs_error"] = serde_json::json!(e.to_string()); }
                    }
                }
                result.to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_read_range", description = "Read a byte range [start, end) from a file. Returns base64-encoded bytes.")]
    async fn read_range(&self, Parameters(params): Parameters<ReadRangeParams>) -> String {
        match self.client.read_file_range(&params.manifest_cid, params.start, params.end).await {
            Ok(data) => {
                use base64::Engine;
                serde_json::json!({
                    "data_base64": base64::engine::general_purpose::STANDARD.encode(&data),
                    "size": data.len(),
                }).to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_pin", description = "Pin a file to prevent garbage collection.")]
    async fn pin(&self, Parameters(params): Parameters<PinParams>) -> String {
        match self.client.pin_file(&params.manifest_cid).await {
            Ok(()) => serde_json::json!({ "manifest_cid": params.manifest_cid, "status": "pinned" }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_unpin", description = "Unpin a file, allowing garbage collection.")]
    async fn unpin(&self, Parameters(params): Parameters<UnpinParams>) -> String {
        match self.client.unpin_file(&params.manifest_cid).await {
            Ok(()) => serde_json::json!({ "manifest_cid": params.manifest_cid, "status": "unpinned" }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_extract_text", description = "Extract text from a file (PDF, DOCX, HTML, Markdown, plain text).")]
    async fn extract_text(&self, Parameters(params): Parameters<ExtractTextParams>) -> String {
        match self.client.extract_text(&params.manifest_cid).await {
            Ok(Some(text)) => serde_json::json!({ "manifest_cid": params.manifest_cid, "text": text }).to_string(),
            Ok(None) => serde_json::json!({ "manifest_cid": params.manifest_cid, "text": null, "note": "unsupported or failed" }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_file_info", description = "Get manifest metadata for a file.")]
    async fn file_info(&self, Parameters(params): Parameters<FileInfoParams>) -> String {
        match self.client.get_file_manifest(&params.manifest_cid).await {
            Ok(Some(m)) => m.to_string(),
            Ok(None) => format!("error: manifest not found for cid {}", params.manifest_cid),
            Err(e) => format!("error: {e}"),
        }
    }

    // ── Graph ──────────────────────────────────────────────────────

    #[tool(name = "memvault_graph_add", description = "Add an entity to the knowledge graph. Returns the hex-encoded entity ID.")]
    async fn graph_add(&self, Parameters(params): Parameters<GraphAddParams>) -> String {
        let vis = params.visibility.as_deref().or(Some(self.default_visibility.as_str()));
        let props = serde_json::json!(params.props);
        match self.client.add_entity(&params.kind, props, vis).await {
            Ok(resp) => {
                let raw_id = resp.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let node_id = ensure_entity_id(raw_id);
                let mut result = serde_json::json!({ "node_id": node_id, "status": "created" });
                if let Some(vfs_path) = &params.vfs_path {
                    let vfs = Vfs::new(self.client.as_ref());
                    match vfs.link(vfs_path, &node_id).await {
                        Ok(_) => { result["vfs_path"] = serde_json::json!(vfs_path); }
                        Err(e) => { result["vfs_error"] = serde_json::json!(e.to_string()); }
                    }
                }
                result.to_string()
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_get_entity", description = "Get a single entity by hex ID. Returns kind, properties, and edges.")]
    async fn get_entity(&self, Parameters(params): Parameters<GetEntityParams>) -> String {
        match self.client.get_entity(&params.id).await {
            Ok(Some(entity)) => entity.to_string(),
            Ok(None) => format!("error: entity not found: {}", params.id),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_list_entities", description = "List knowledge graph entities.")]
    async fn list_entities(&self, Parameters(params): Parameters<ListEntitiesParams>) -> String {
        ok_or_err!(self.client.list_entities(params.limit.unwrap_or(50)).await)
    }

    #[tool(name = "memvault_traverse", description = "Traverse the graph from any node (type:hex). Returns connected nodes up to max_depth.")]
    async fn traverse(&self, Parameters(params): Parameters<TraverseParams>) -> String {
        ok_or_err!(self.client.traverse_from(&params.from, params.relation.as_deref(), params.max_depth.unwrap_or(2)).await)
    }

    #[tool(name = "memvault_graph_link", description = "Link two entities by hex ID. Use memvault_link for cross-type linking.")]
    async fn graph_link(&self, Parameters(params): Parameters<GraphLinkParams>) -> String {
        let source = ensure_entity_id(&params.source_id);
        let target = ensure_entity_id(&params.target_id);
        let props = params.props.into_iter().map(|(k, v)| (k, v)).collect();
        match self.client.add_link(&source, &target, &params.relation, params.weight, props).await {
            Ok(resp) => serde_json::json!({
                "edge_id": resp.get("edge_id").and_then(|v| v.as_str()).unwrap_or(""),
                "status": "linked"
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_graph_query", description = "List all edges for an entity.")]
    async fn graph_query(&self, Parameters(params): Parameters<GraphQueryParams>) -> String {
        ok_or_err!(self.client.edges_of(&ensure_entity_id(&params.from_id)).await)
    }

    // ── Links (cross-type) ─────────────────────────────────────────

    #[tool(name = "memvault_link", description = "Link any two nodes (type:hex format). Returns the edge ID.")]
    async fn link(&self, Parameters(params): Parameters<LinkParams>) -> String {
        let props = params.props.into_iter().map(|(k, v)| (k, v)).collect();
        match self.client.add_link(&params.source, &params.target, &params.relation, params.weight, props).await {
            Ok(resp) => serde_json::json!({
                "edge_id": resp.get("edge_id").and_then(|v| v.as_str()).unwrap_or(""),
                "status": "linked"
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(name = "memvault_edges", description = "List all edges for any node (type:hex).")]
    async fn edges(&self, Parameters(params): Parameters<EdgesOfParams>) -> String {
        ok_or_err!(self.client.edges_of(&params.node).await)
    }

    #[tool(name = "memvault_unlink", description = "Remove an edge. Requires edge_id and source node (type:hex).")]
    async fn unlink(&self, Parameters(params): Parameters<UnlinkParams>) -> String {
        match self.client.delete_link(&params.edge_id, &params.source).await {
            Ok(_) => serde_json::json!({ "edge_id": params.edge_id, "status": "removed" }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    // ── Nodes (universal) ──────────────────────────────────────────

    #[tool(name = "memvault_list_all", description = "List all nodes (docs, entities, files). Optionally filter by view name.")]
    async fn list_all(&self, Parameters(params): Parameters<ListAllParams>) -> String {
        ok_or_err!(self.client.list_all(params.view.as_deref(), params.limit.unwrap_or(100)).await)
    }

    #[tool(name = "memvault_retract", description = "Retract (soft-delete) any node. Node must be in type:hex format: doc:<hex>, entity:<hex>, or file:<hex>.")]
    async fn retract(&self, Parameters(params): Parameters<RetractParams>) -> String {
        ok_or_err!(self.client.retract_node(&params.node, &params.reason).await)
    }

    // ── Tags ───────────────────────────────────────────────────────

    #[tool(name = "memvault_tag", description = "Add tags to an item (type:hex node ID). Tags in scope:label format.")]
    async fn tag(&self, Parameters(params): Parameters<TagParams>) -> String {
        ok_or_err!(self.client.add_tags(&params.node, parse_tags(&params.tags)).await)
    }

    #[tool(name = "memvault_untag", description = "Remove tags from an item. Tags in scope:label format.")]
    async fn untag(&self, Parameters(params): Parameters<UntagParams>) -> String {
        ok_or_err!(self.client.remove_tags(&params.node, parse_tags(&params.tags)).await)
    }

    #[tool(name = "memvault_get_tags", description = "Get effective tags for an item.")]
    async fn get_tags(&self, Parameters(params): Parameters<GetTagsParams>) -> String {
        ok_or_err!(self.client.get_tags(&params.node).await)
    }

    // ── Views ──────────────────────────────────────────────────────

    #[tool(name = "memvault_view_list", description = "List all saved views.")]
    async fn view_list(&self) -> String {
        ok_or_err!(self.client.list_views().await)
    }

    #[tool(name = "memvault_view_create", description = "Create a view. Tags in scope:label format.")]
    async fn view_create(&self, Parameters(params): Parameters<ViewCreateParams>) -> String {
        ok_or_err!(self.client.create_view(&params.name, parse_tags(&params.tags)).await)
    }

    #[tool(name = "memvault_view_update", description = "Update a view's tags.")]
    async fn view_update(&self, Parameters(params): Parameters<ViewUpdateParams>) -> String {
        ok_or_err!(self.client.update_view(&params.name, parse_tags(&params.tags)).await)
    }

    #[tool(name = "memvault_view_delete", description = "Delete a view.")]
    async fn view_delete(&self, Parameters(params): Parameters<ViewDeleteParams>) -> String {
        ok_or_err!(self.client.delete_view(&params.name).await)
    }

    // ── Audit ──────────────────────────────────────────────────────

    #[tool(name = "memvault_audit", description = "Query audit log. Optionally filter by op_kind (DocCreate, EntityCreate, AttachFile, EdgeAdd, Retract).")]
    async fn audit(&self, Parameters(params): Parameters<AuditParams>) -> String {
        ok_or_err!(self.client.audit(params.limit.unwrap_or(50), params.op_kind.as_deref()).await)
    }

    // ── Status ─────────────────────────────────────────────────────

    #[tool(name = "memvault_status", description = "Get node status (block count, doc count, peer count, uptime).")]
    async fn status(&self) -> String {
        ok_or_err!(self.client.status().await)
    }

    // ── VFS (Virtual Filesystem) ──────────────────────────────────

    #[tool(
        name = "memvault_vfs_ls",
        description = "List directory contents at a VFS path. Shows name, type, and node ID for each entry."
    )]
    async fn vfs_ls(&self, Parameters(params): Parameters<VfsLsParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        let recursive = params.recursive.unwrap_or(false);
        match vfs.ls(&params.path, recursive).await {
            Ok(entries) => serde_json::json!({
                "path": params.path,
                "entries": entries,
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_vfs_resolve",
        description = "Resolve a VFS path to its target node ID (type:hex format)."
    )]
    async fn vfs_resolve(&self, Parameters(params): Parameters<VfsResolveParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        match vfs.resolve(&params.path).await {
            Ok(Some((node_id, edge_id))) => serde_json::json!({
                "path": params.path,
                "node_id": node_id,
                "edge_id": edge_id,
            }).to_string(),
            Ok(None) => serde_json::json!({ "path": params.path, "error": "not found" }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_vfs_mkdir",
        description = "Create a directory at a VFS path. Intermediate directories are created automatically (like mkdir -p)."
    )]
    async fn vfs_mkdir(&self, Parameters(params): Parameters<VfsMkdirParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        match vfs.mkdir(&params.path).await {
            Ok(entity_id) => serde_json::json!({
                "path": params.path,
                "entity_id": entity_id,
                "status": "created",
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_vfs_link",
        description = "Place a node at a VFS path. Intermediate directories are created automatically. A node can appear at multiple paths."
    )]
    async fn vfs_link(&self, Parameters(params): Parameters<VfsLinkParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        match vfs.link(&params.path, &params.target).await {
            Ok(edge_id) => serde_json::json!({
                "path": params.path,
                "target": params.target,
                "edge_id": edge_id,
                "status": "linked",
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_vfs_unlink",
        description = "Remove an entry from a VFS path. The underlying node is NOT deleted — only the VFS link is removed."
    )]
    async fn vfs_unlink(&self, Parameters(params): Parameters<VfsUnlinkParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        match vfs.unlink(&params.path).await {
            Ok(()) => serde_json::json!({
                "path": params.path,
                "status": "unlinked",
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_vfs_mv",
        description = "Move or rename a VFS entry from one path to another."
    )]
    async fn vfs_mv(&self, Parameters(params): Parameters<VfsMvParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        match vfs.mv(&params.from, &params.to).await {
            Ok(()) => serde_json::json!({
                "from": params.from,
                "to": params.to,
                "status": "moved",
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_vfs_tree",
        description = "Display an ASCII tree view of the VFS hierarchy from a given path."
    )]
    async fn vfs_tree(&self, Parameters(params): Parameters<VfsTreeParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        let path = params.path.as_deref().unwrap_or("/");
        let max_depth = params.max_depth.unwrap_or(5);
        match vfs.tree(path, max_depth).await {
            Ok(tree) => tree,
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_vfs_find",
        description = "Find all VFS paths that link to a given node. Useful for discovering where a node is mounted."
    )]
    async fn vfs_find(&self, Parameters(params): Parameters<VfsFindParams>) -> String {
        let vfs = Vfs::new(self.client.as_ref());
        match vfs.find_paths(&params.node).await {
            Ok(paths) => serde_json::json!({
                "node": params.node,
                "paths": paths,
            }).to_string(),
            Err(e) => format!("error: {e}"),
        }
    }

    // ── Export ─────────────────────────────────────────────────────

    #[tool(
        name = "memvault_export",
        description = "Export a single node (document, file, or entity) to a temp file. \
                       Pass node_id as 'doc:<hex>', 'entity:<hex>', or 'file:<hex>'. \
                       Returns the file path. For documents, optionally includes history."
    )]
    async fn export_node(&self, Parameters(params): Parameters<ExportNodeParams>) -> String {
        let Some(client) = self.client.as_memvault_client() else {
            return "error: export tools require local mode (--db)".to_string();
        };
        let out_dir = std::env::temp_dir().join("memvault-export");
        let history = params.history.unwrap_or(false);
        match memvault_export::export_node(client, &params.node_id, &out_dir, history).await {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|e| format!("error: {e}")),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        name = "memvault_export_vault",
        description = "Export the entire vault (or a filtered subset) to a directory or tar archive on disk."
    )]
    async fn export_vault(&self, Parameters(params): Parameters<ExportVaultParams>) -> String {
        let Some(client) = self.client.as_memvault_client() else {
            return "error: export tools require local mode (--db)".to_string();
        };
        let tag_filter = params.tag.as_deref().and_then(|t| {
            let parts: Vec<&str> = t.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        });
        let opts = memvault_export::ExportOptions {
            history: params.history.unwrap_or(false),
            include_vfs: true,
            tag_filter,
            view_filter: params.view,
        };
        let output = std::path::PathBuf::from(&params.output_path);
        let force_tar = params.tar.unwrap_or(false);
        let gzip = output.to_str().is_some_and(|s| s.ends_with(".gz") || s.ends_with(".tgz"));
        let sink = match memvault_export::create_sink(&output, force_tar, gzip) {
            Ok(s) => s,
            Err(e) => return format!("error creating output: {e}"),
        };
        match memvault_export::run_export(client, sink, opts).await {
            Ok(stats) => format!(
                "Exported {} documents, {} files, {} entities ({} history versions) to {}",
                stats.documents, stats.files, stats.entities, stats.history_versions,
                params.output_path
            ),
            Err(e) => format!("error: {e}"),
        }
    }
}
