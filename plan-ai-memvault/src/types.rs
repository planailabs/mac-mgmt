use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;

// -- memvault_put --

#[derive(Deserialize, JsonSchema)]
pub struct PutParams {
    /// The document text to store.
    pub text: String,
    /// Optional title for the document.
    #[serde(default)]
    pub title: Option<String>,
    /// Tags in "scope:label" format.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Visibility level: "internal", "cluster", or "public". Defaults to "internal".
    #[serde(default)]
    pub visibility: Option<String>,
}

// -- memvault_get --

#[derive(Deserialize, JsonSchema)]
pub struct GetParams {
    /// Hex-encoded CID of the document.
    pub cid: String,
}

// -- memvault_search --

#[derive(Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Search query text.
    pub query: String,
    /// Maximum number of results (default: 10).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Optional tag filter in "scope:label" format.
    #[serde(default)]
    pub tag_filter: Option<String>,
}

// -- memvault_list --

#[derive(Deserialize, JsonSchema)]
pub struct ListParams {
    /// Maximum number of results (default: 20).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Filter by tag scope.
    #[serde(default)]
    pub tag_scope: Option<String>,
    /// Filter by tag label.
    #[serde(default)]
    pub tag_label: Option<String>,
}

// -- memvault_attach --

#[derive(Deserialize, JsonSchema)]
pub struct AttachParams {
    /// Filename for the attachment.
    pub filename: String,
    /// Base64-encoded file content.
    pub content_base64: String,
    /// MIME content type (default: "application/octet-stream").
    #[serde(default)]
    pub content_type: Option<String>,
    /// Tags in "scope:label" format.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Visibility level: "internal", "federated", or "public". Defaults to "internal".
    #[serde(default)]
    pub visibility: Option<String>,
}

// -- memvault_read_range --

#[derive(Deserialize, JsonSchema)]
pub struct ReadRangeParams {
    /// Hex-encoded manifest CID.
    pub manifest_cid: String,
    /// Start byte offset (inclusive).
    pub start: u64,
    /// End byte offset (exclusive).
    pub end: u64,
}

// -- memvault_pin --

#[derive(Deserialize, JsonSchema)]
pub struct PinParams {
    /// Hex-encoded manifest CID of the attachment to pin.
    pub manifest_cid: String,
}

// -- memvault_unpin --

#[derive(Deserialize, JsonSchema)]
pub struct UnpinParams {
    /// Hex-encoded manifest CID of the attachment to unpin.
    pub manifest_cid: String,
}

// -- memvault_extract_text --

#[derive(Deserialize, JsonSchema)]
pub struct ExtractTextParams {
    /// Hex-encoded manifest CID of the attachment to extract text from.
    pub manifest_cid: String,
}

// -- memvault_attachment_info --

#[derive(Deserialize, JsonSchema)]
pub struct AttachmentInfoParams {
    /// Hex-encoded manifest CID of the attachment.
    pub manifest_cid: String,
}

// -- memvault_graph_add --

#[derive(Deserialize, JsonSchema)]
pub struct GraphAddParams {
    /// Entity kind/type (e.g. "person", "project", "concept").
    pub kind: String,
    /// Key-value properties for the entity.
    #[serde(default)]
    pub props: HashMap<String, String>,
    /// Visibility level. Defaults to "internal".
    #[serde(default)]
    pub visibility: Option<String>,
}

// -- memvault_graph_link --

#[derive(Deserialize, JsonSchema)]
pub struct GraphLinkParams {
    /// Hex-encoded source entity ID.
    pub source_id: String,
    /// Hex-encoded target entity ID.
    pub target_id: String,
    /// Relation type (e.g. "knows", "depends_on", "part_of").
    pub relation: String,
    /// Optional edge weight (0.0 to 1.0).
    #[serde(default)]
    pub weight: Option<f32>,
}

// -- memvault_graph_query --

#[derive(Deserialize, JsonSchema)]
pub struct GraphQueryParams {
    /// Hex-encoded entity ID to start traversal from.
    pub from_id: String,
    /// Optional relation filter.
    #[serde(default)]
    pub relation: Option<String>,
    /// Maximum traversal depth (default: 2).
    #[serde(default)]
    pub max_depth: Option<usize>,
}

// -- memvault_retract --

#[derive(Deserialize, JsonSchema)]
pub struct RetractParams {
    /// Hex-encoded CID of the memory to retract.
    pub cid: String,
    /// Reason for retraction.
    pub reason: String,
}
