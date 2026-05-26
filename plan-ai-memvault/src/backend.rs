//! Backend trait — abstracts over HTTP and local (direct redb) access.

use anyhow::Result;
use async_trait::async_trait;
use memvault_api::MemvaultClient;

/// Operations the MCP server needs from its backend.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Return the underlying MemvaultClient if available (local mode).
    /// Used by export tools that need typed access.
    fn as_memvault_client(&self) -> Option<&dyn MemvaultClient> {
        None
    }

    // -- Documents --
    async fn put_doc(&self, body: &str, frontmatter: serde_json::Value, tags: Vec<(String, String)>, visibility: Option<&str>) -> Result<serde_json::Value>;
    async fn get_doc(&self, id: &str) -> Result<Option<serde_json::Value>>;
    async fn edit_doc(&self, id: &str, patch_json: serde_json::Value) -> Result<serde_json::Value>;
    async fn list_docs(&self, tag_ns: Option<&str>, tag_val: Option<&str>, limit: usize) -> Result<serde_json::Value>;
    async fn history_of(&self, doc_id: &str) -> Result<serde_json::Value>;

    // -- Files --
    async fn upload_file(&self, data: &[u8], filename: &str, content_type: &str) -> Result<serde_json::Value>;
    async fn download_file(&self, cid_hex: &str) -> Result<Vec<u8>>;
    async fn read_file_range(&self, cid_hex: &str, start: u64, end: u64) -> Result<Vec<u8>>;
    async fn extract_text(&self, cid_hex: &str) -> Result<Option<String>>;
    async fn get_file_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>>;
    async fn pin_file(&self, cid_hex: &str) -> Result<()>;
    async fn unpin_file(&self, cid_hex: &str) -> Result<()>;

    // -- Graph --
    async fn add_entity(&self, kind: &str, props: serde_json::Value, visibility: Option<&str>) -> Result<serde_json::Value>;
    async fn get_entity(&self, id: &str) -> Result<Option<serde_json::Value>>;
    async fn list_entities(&self, limit: usize) -> Result<serde_json::Value>;
    async fn entity_history(&self, id: &str) -> Result<serde_json::Value>;
    async fn traverse_from(&self, from: &str, relation: Option<&str>, max_depth: usize) -> Result<serde_json::Value>;

    // -- Links --
    async fn add_link(&self, source: &str, target: &str, relation: &str, weight: Option<f32>, props: std::collections::BTreeMap<String, serde_json::Value>) -> Result<serde_json::Value>;
    async fn edges_of(&self, node: &str) -> Result<serde_json::Value>;
    async fn delete_link(&self, edge_id: &str, source: &str) -> Result<serde_json::Value>;

    // -- Search --
    async fn search(&self, query: &str, limit: usize, tag_filter: Option<&str>) -> Result<serde_json::Value>;
    async fn search_unified(&self, query: &str, limit: usize) -> Result<serde_json::Value>;
    async fn resolve_label(&self, node_id: &str) -> Result<Option<String>>;

    // -- Nodes (universal) --
    async fn list_all(&self, view: Option<&str>, limit: usize) -> Result<serde_json::Value>;
    async fn retract(&self, cid_hex: &str, reason: &str) -> Result<serde_json::Value>;
    async fn retract_node(&self, node_id: &str, reason: &str) -> Result<serde_json::Value>;

    // -- Tags --
    async fn add_tags(&self, node_id: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn remove_tags(&self, node_id: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn get_tags(&self, node_id: &str) -> Result<serde_json::Value>;

    // -- Views --
    async fn list_views(&self) -> Result<serde_json::Value>;
    async fn create_view(&self, name: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn update_view(&self, name: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn delete_view(&self, name: &str) -> Result<serde_json::Value>;
    async fn view_members(&self, name: &str) -> Result<serde_json::Value>;

    // -- Buckets --
    async fn bucket_list(&self) -> Result<serde_json::Value>;
    async fn bucket_create(&self, name: &str, description: Option<&str>) -> Result<serde_json::Value>;
    async fn bucket_get(&self, id: &str) -> Result<Option<serde_json::Value>>;
    async fn bucket_rename(&self, id: &str, new_name: &str) -> Result<serde_json::Value>;
    async fn bucket_attach(&self, id: &str) -> Result<serde_json::Value>;
    async fn bucket_archive(&self, id: &str, reason: &str) -> Result<serde_json::Value>;

    // -- Audit --
    async fn audit(&self, limit: usize, op_kind: Option<&str>) -> Result<serde_json::Value>;

    // -- Admin --
    async fn status(&self) -> Result<serde_json::Value>;
}
