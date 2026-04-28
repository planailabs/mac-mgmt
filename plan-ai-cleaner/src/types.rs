use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A redaction session — tracks detected entities, approval state, and the
/// redacted document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionManifest {
    pub id: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub ttl_seconds: u64,
    pub status: SessionStatus,
    /// SHA-256 hex digest of the original document text.
    pub original_hash: String,
    /// Detected entities (pre-approval: all found; post-approval: filtered).
    pub entities: Vec<RedactionEntity>,
    /// The fully redacted document text — populated after approval.
    pub redacted_text: Option<String>,
    /// The original document text — kept for applying redactions.
    pub original_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Scanned,
    Approved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionEntity {
    /// Unique placeholder id, e.g. "PERSON_1", "EMAIL_2".
    pub id: String,
    pub category: EntityCategory,
    /// The original sensitive value.
    pub original: String,
    /// The placeholder string, e.g. "[PERSON_1]".
    pub placeholder: String,
    pub source: DetectionSource,
    /// Whether this entity is approved for redaction.
    pub approved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityCategory {
    Person,
    Email,
    Phone,
    CreditCard,
    Ssn,
    IpAddress,
    ApiKey,
    Jwt,
    Company,
    ProjectName,
    InternalUrl,
    Proprietary,
    Custom,
}

impl EntityCategory {
    pub fn prefix(&self) -> &str {
        match self {
            Self::Person => "PERSON",
            Self::Email => "EMAIL",
            Self::Phone => "PHONE",
            Self::CreditCard => "CC",
            Self::Ssn => "SSN",
            Self::IpAddress => "IP",
            Self::ApiKey => "APIKEY",
            Self::Jwt => "JWT",
            Self::Company => "COMPANY",
            Self::ProjectName => "PROJECT",
            Self::InternalUrl => "URL",
            Self::Proprietary => "PROPRIETARY",
            Self::Custom => "CUSTOM",
        }
    }
}

impl std::fmt::Display for EntityCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.prefix())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionSource {
    Regex,
    Llm,
    Manual,
}

impl std::fmt::Display for DetectionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Regex => f.write_str("regex"),
            Self::Llm => f.write_str("llm"),
            Self::Manual => f.write_str("manual"),
        }
    }
}

// ── MCP tool parameter structs ──────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct ScanParams {
    /// The document text to scan for PII and secrets.
    pub text: String,
    /// Optional human-friendly session name.
    #[serde(default)]
    pub session_name: Option<String>,
    /// Whether to use the local LLM (Ollama) for contextual detection.
    /// Defaults to true. Set to false for regex-only scanning (faster).
    #[serde(default = "default_true")]
    pub use_llm: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize, JsonSchema)]
pub struct ApproveParams {
    /// The session ID returned by cleaner_scan.
    pub session_id: String,
    /// Entity IDs to exclude from redaction (false positives).
    #[serde(default)]
    pub remove_ids: Vec<String>,
    /// Additional text strings to manually redact.
    #[serde(default)]
    pub add_redactions: Vec<ManualRedaction>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ManualRedaction {
    /// The exact text to redact.
    pub text: String,
    /// Category label for the placeholder (defaults to "custom").
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct RehydrateParams {
    /// The session ID whose manifest to use for rehydration.
    pub session_id: String,
    /// The text containing placeholders to replace with originals.
    pub text: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SessionIdParam {
    /// The session ID.
    pub session_id: String,
}
