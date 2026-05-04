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
    /// Absolute path to the file on the local filesystem.
    pub path: String,
    /// MIME content type. If omitted, guessed from the file extension.
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

// -- memvault_link --

#[derive(Deserialize, JsonSchema)]
pub struct LinkParams {
    /// Source node as "entity:<hex>", "doc:<hex>", or "attachment:<hex>".
    pub source: String,
    /// Target node — same format as source.
    pub target: String,
    /// Relation type (e.g. "references", "evidence_for", "related_to").
    pub relation: String,
    /// Optional edge weight (0.0 to 1.0).
    #[serde(default)]
    pub weight: Option<f32>,
}

// -- memvault_edges --

#[derive(Deserialize, JsonSchema)]
pub struct EdgesOfParams {
    /// Node to query — "entity:<hex>", "doc:<hex>", or "attachment:<hex>".
    pub node: String,
}

// -- memvault_unlink --

#[derive(Deserialize, JsonSchema)]
pub struct UnlinkParams {
    /// Hex-encoded edge ID to remove.
    pub edge_id: String,
    /// Source node — "entity:<hex>", "doc:<hex>", or "attachment:<hex>".
    pub source: String,
}

// -- memvault_tag --

#[derive(Deserialize, JsonSchema)]
pub struct TagParams {
    /// Node to tag — "entity:<hex>", "doc:<hex>", or "attachment:<hex>".
    pub node: String,
    /// Tags to add in "scope:label" format.
    pub tags: Vec<String>,
}

// -- memvault_untag --

#[derive(Deserialize, JsonSchema)]
pub struct UntagParams {
    /// Node to untag — "entity:<hex>", "doc:<hex>", or "attachment:<hex>".
    pub node: String,
    /// Tags to remove in "scope:label" format.
    pub tags: Vec<String>,
}

// -- memvault_get_tags --

#[derive(Deserialize, JsonSchema)]
pub struct GetTagsParams {
    /// Node — "entity:<hex>", "doc:<hex>", or "attachment:<hex>".
    pub node: String,
}

// -- memvault_view_list --
// (no params)

// -- memvault_view_create --

#[derive(Deserialize, JsonSchema)]
pub struct ViewCreateParams {
    /// Name of the view.
    pub name: String,
    /// Required tags in "scope:label" format. Items must have ALL of these to appear.
    pub tags: Vec<String>,
}

// -- memvault_view_update --

#[derive(Deserialize, JsonSchema)]
pub struct ViewUpdateParams {
    /// Name of the view to update.
    pub name: String,
    /// New set of required tags in "scope:label" format.
    pub tags: Vec<String>,
}

// -- memvault_view_delete --

#[derive(Deserialize, JsonSchema)]
pub struct ViewDeleteParams {
    /// Name of the view to delete.
    pub name: String,
}

// -- memvault_retract --

#[derive(Deserialize, JsonSchema)]
pub struct RetractParams {
    /// Hex-encoded CID of the memory to retract.
    pub cid: String,
    /// Reason for retraction.
    pub reason: String,
}
