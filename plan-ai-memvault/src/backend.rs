//! Backend trait — abstracts over HTTP and local (direct redb) access.

use anyhow::Result;
use async_trait::async_trait;

/// Operations the MCP server needs from its backend.
#[async_trait]
pub trait Backend: Send + Sync {
    async fn put_doc(
        &self, body: &str, frontmatter: serde_json::Value,
        tags: Vec<(String, String)>, visibility: Option<&str>,
    ) -> Result<serde_json::Value>;

    async fn get_doc(&self, id: &str) -> Result<Option<serde_json::Value>>;

    async fn search(&self, query: &str, limit: usize, tag_filter: Option<&str>) -> Result<serde_json::Value>;

    async fn list_docs(
        &self, tag_ns: Option<&str>, tag_val: Option<&str>, limit: usize,
    ) -> Result<serde_json::Value>;

    async fn attach_file(&self, data: &[u8], filename: &str, content_type: &str) -> Result<serde_json::Value>;

    async fn download_attachment(&self, cid_hex: &str) -> Result<Vec<u8>>;

    async fn read_attachment_range(&self, cid_hex: &str, start: u64, end: u64) -> Result<Vec<u8>>;

    async fn extract_text(&self, cid_hex: &str) -> Result<Option<String>>;

    async fn get_attachment_manifest(&self, cid_hex: &str) -> Result<Option<serde_json::Value>>;

    async fn pin_attachment(&self, cid_hex: &str) -> Result<()>;

    async fn unpin_attachment(&self, cid_hex: &str) -> Result<()>;

    async fn add_entity(
        &self, kind: &str, props: serde_json::Value, visibility: Option<&str>,
    ) -> Result<serde_json::Value>;

    async fn add_link(
        &self, source: &str, target: &str, relation: &str, weight: Option<f32>,
    ) -> Result<serde_json::Value>;

    async fn edges_of(&self, node: &str) -> Result<serde_json::Value>;

    async fn delete_link(&self, edge_id: &str, source: &str) -> Result<serde_json::Value>;

    async fn retract(&self, cid_hex: &str, reason: &str) -> Result<serde_json::Value>;
    async fn retract_node(&self, node_id: &str, reason: &str) -> Result<serde_json::Value>;

    async fn add_tags(&self, node_id: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn remove_tags(&self, node_id: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn get_tags(&self, node_id: &str) -> Result<serde_json::Value>;

    async fn list_views(&self) -> Result<serde_json::Value>;
    async fn create_view(&self, name: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn update_view(&self, name: &str, tags: Vec<(String, String)>) -> Result<serde_json::Value>;
    async fn delete_view(&self, name: &str) -> Result<serde_json::Value>;

    async fn status(&self) -> Result<serde_json::Value>;
}
