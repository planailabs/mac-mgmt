//! Built-in skill center providing plan-ai MCP servers and embedded skills.
//!
//! Instead of fetching from a remote federation endpoint, the catalog and MCP
//! configs are generated locally.  Skill content is embedded from the repo-root
//! `skills/` directory via `rust_embed` and SKILL.md frontmatter is parsed at
//! runtime to populate the federation catalog.
//!
//! The skill center row is seeded by migration 056 and its catalog is injected
//! into the `SkillCenterCache` at startup.

use std::collections::HashMap;

use mac_mgmt_common::{
    FederationBundle, FederationBundleSkill, FederationCatalog, FederationMcpBundle,
    FederationMcpBundleServer, FederationMcpServer, FederationSkillChannel, McpServerEntry,
};
use uuid::Uuid;

/// Well-known UUID for the built-in skill center (matches migration 056).
pub const BUILTIN_SKILL_CENTER_ID: Uuid = Uuid::from_bytes([
    0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01,
]);

/// Well-known UUID for the plan-ai-cleaner MCP server.
const CLEANER_ID: Uuid = Uuid::from_bytes([
    0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02,
]);

/// Well-known UUID for the plan-ai-cloud MCP server.
const CLOUD_ID: Uuid = Uuid::from_bytes([
    0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x03,
]);

/// Well-known UUID for the built-in MCP bundle.
const MCP_BUNDLE_ID: Uuid = Uuid::from_bytes([
    0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x04,
]);

/// Well-known UUID for the built-in skills bundle.
const SKILLS_BUNDLE_ID: Uuid = Uuid::from_bytes([
    0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x06,
]);

pub const BUILTIN_URL: &str = "builtin://";

// ── Embedded skills ──────────────────────────────────────────────────────

#[derive(rust_embed::Embed)]
#[folder = "../skills/"]
#[include = "*/SKILL.md"]
struct BuiltinSkills;

/// Parsed SKILL.md frontmatter.
struct SkillMeta {
    slug: String,
    name: String,
    description: String,
    tools: Vec<String>,
}

/// Parse YAML frontmatter from a SKILL.md file.
/// Extracts `name`, `description`, and `tools` fields.
fn parse_frontmatter(raw: &str) -> Option<SkillMeta> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let end = trimmed[3..].find("\n---")?;
    let fm_block = &trimmed[3..3 + end];

    let mut name = String::new();
    let mut description = String::new();
    let mut tools = Vec::new();
    let mut in_tools = false;

    for line in fm_block.lines() {
        let line_trimmed = line.trim();
        if let Some(v) = line_trimmed.strip_prefix("name:") {
            name = v.trim().to_string();
            in_tools = false;
        } else if let Some(v) = line_trimmed.strip_prefix("description:") {
            description = v.trim().to_string();
            in_tools = false;
        } else if line_trimmed.starts_with("tools:") {
            in_tools = true;
        } else if in_tools {
            if let Some(item) = line_trimmed.strip_prefix("- ") {
                tools.push(item.trim().to_string());
            } else if !line_trimmed.is_empty() {
                in_tools = false;
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(SkillMeta {
        slug: name.clone(),
        name,
        description,
        tools,
    })
}

/// Generate a deterministic UUID v5-style ID for a built-in skill slug.
/// Uses the skill center UUID as namespace and a simple hash of the slug.
fn skill_uuid(slug: &str) -> Uuid {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(BUILTIN_SKILL_CENTER_ID.as_bytes());
    hasher.update(b"skill:");
    hasher.update(slug.as_bytes());
    let hash = hasher.finalize();
    // Take first 16 bytes and set version/variant bits for UUID v5-ish
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50; // version 5
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant RFC 4122
    Uuid::from_bytes(bytes)
}

/// List all embedded skill slugs.
fn embedded_skills() -> Vec<SkillMeta> {
    let mut skills = Vec::new();
    for path in BuiltinSkills::iter() {
        // Paths are like "pii-redact/SKILL.md"
        let path_str = path.as_ref();
        if !path_str.ends_with("/SKILL.md") {
            continue;
        }
        let slug = match path_str.strip_suffix("/SKILL.md") {
            Some(s) => s.to_string(),
            None => continue,
        };
        if let Some(file) = BuiltinSkills::get(path_str) {
            if let Ok(content) = std::str::from_utf8(&file.data) {
                if let Some(mut meta) = parse_frontmatter(content) {
                    meta.slug = slug;
                    skills.push(meta);
                }
            }
        }
    }
    skills
}

// ── Public API ────────────────────────────────────────────────────────────

pub fn is_builtin(id: &Uuid) -> bool {
    *id == BUILTIN_SKILL_CENTER_ID
}

/// Build the federation catalog for built-in MCP servers and skills.
pub fn builtin_catalog() -> FederationCatalog {
    let skills = embedded_skills();

    let skill_channels: Vec<FederationSkillChannel> = skills
        .iter()
        .map(|s| FederationSkillChannel {
            id: skill_uuid(&s.slug),
            skill_slug: s.slug.clone(),
            skill_name: s.name.clone(),
            skill_description: s.description.clone(),
            channel: "builtin".into(),
            hidden: false,
            nix_packages: vec![],
            mcp_server_slugs: s.tools.clone(),
        })
        .collect();

    let bundle_skills: Vec<FederationBundleSkill> = skills
        .iter()
        .map(|s| FederationBundleSkill {
            skill_channel_id: skill_uuid(&s.slug),
            skill_slug: s.slug.clone(),
            channel: "builtin".into(),
        })
        .collect();

    let bundles = if bundle_skills.is_empty() {
        vec![]
    } else {
        vec![FederationBundle {
            id: SKILLS_BUNDLE_ID,
            slug: "plan-ai-builtin-skills".into(),
            name: "Built-in Skills".into(),
            description: "Bundle containing built-in skills shipped with the daemon".into(),
            hidden: false,
            skills: bundle_skills,
        }]
    };

    FederationCatalog {
        skill_channels,
        bundles,
        mcp_servers: vec![
            FederationMcpServer {
                id: CLEANER_ID,
                slug: "plan-ai-cleaner".into(),
                name: "PII/Secret Cleaner".into(),
                description: "PII and secret detection/redaction MCP server".into(),
                hidden: false,
                config: cleaner_config(),
                nix_packages: vec![],
            },
            FederationMcpServer {
                id: CLOUD_ID,
                slug: "plan-ai-cloud".into(),
                name: "Multi-Cloud LLM".into(),
                description: "MCP server for querying cloud LLMs via LiteLLM with cleaner integration"
                    .into(),
                hidden: false,
                config: cloud_config(),
                nix_packages: vec![],
            },
        ],
        mcp_bundles: vec![FederationMcpBundle {
            id: MCP_BUNDLE_ID,
            slug: "plan-ai-builtin".into(),
            name: "Built-in MCP Servers".into(),
            description: "Bundle containing the built-in cleaner and cloud MCP servers".into(),
            hidden: false,
            servers: vec![
                FederationMcpBundleServer {
                    mcp_server_id: CLEANER_ID,
                    slug: "plan-ai-cleaner".into(),
                },
                FederationMcpBundleServer {
                    mcp_server_id: CLOUD_ID,
                    slug: "plan-ai-cloud".into(),
                },
            ],
        }],
    }
}

/// Resolve requested slugs to their MCP server configs (local, no HTTP).
pub fn resolve_builtin_mcp_servers(slugs: &[String]) -> HashMap<String, McpServerEntry> {
    let mut result = HashMap::new();
    for slug in slugs {
        let entry = match slug.as_str() {
            "plan-ai-cleaner" => Some(McpServerEntry {
                config: cleaner_config(),
                nix_packages: vec![],
            }),
            "plan-ai-cloud" => Some(McpServerEntry {
                config: cloud_config(),
                nix_packages: vec![],
            }),
            _ => None,
        };
        if let Some(e) = entry {
            result.insert(slug.clone(), e);
        }
    }
    result
}

/// Resolve requested skill slug+channel pairs to `builtin://` store paths (local, no HTTP).
pub fn resolve_builtin_skills(
    slug_channels: &[(String, String)],
) -> HashMap<String, String> {
    let known: Vec<String> = embedded_skills().into_iter().map(|s| s.slug).collect();
    let mut result = HashMap::new();
    for (slug, _channel) in slug_channels {
        if known.iter().any(|k| k == slug) {
            result.insert(slug.clone(), format!("builtin://{slug}"));
        }
    }
    result
}

fn cleaner_config() -> serde_json::Value {
    serde_json::json!({
        "command": "mac-mgmt",
        "args": ["mcp-cleaner"]
    })
}

fn cloud_config() -> serde_json::Value {
    serde_json::json!({
        "command": "mac-mgmt",
        "args": ["mcp-cloud"]
    })
}
