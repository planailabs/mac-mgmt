//! Built-in skill center providing plan-ai-cleaner and plan-ai-cloud MCP servers.
//!
//! Instead of fetching from a remote federation endpoint, the catalog and MCP
//! configs are generated locally. The skill center row is seeded by migration
//! 056 and its catalog is injected into the `SkillCenterCache` at startup.

use std::collections::HashMap;

use mac_mgmt_common::{
    FederationCatalog, FederationMcpBundle, FederationMcpBundleServer, FederationMcpServer,
    McpServerEntry,
};
use uuid::Uuid;

/// Well-known UUID for the built-in skill center (matches migration 056).
pub const BUILTIN_SKILL_CENTER_ID: Uuid =
    Uuid::from_bytes([0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]);

/// Well-known UUID for the plan-ai-cleaner MCP server.
const CLEANER_ID: Uuid =
    Uuid::from_bytes([0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]);

/// Well-known UUID for the plan-ai-cloud MCP server.
const CLOUD_ID: Uuid =
    Uuid::from_bytes([0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03]);

/// Well-known UUID for the built-in MCP bundle.
const BUNDLE_ID: Uuid =
    Uuid::from_bytes([0x00, 0xb1, 0x71, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04]);

pub const BUILTIN_URL: &str = "builtin://";

pub fn is_builtin(id: &Uuid) -> bool {
    *id == BUILTIN_SKILL_CENTER_ID
}

/// Build the federation catalog for built-in MCP servers.
pub fn builtin_catalog() -> FederationCatalog {
    FederationCatalog {
        skill_channels: vec![],
        bundles: vec![],
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
                description: "MCP server for querying cloud LLMs via LiteLLM with cleaner integration".into(),
                hidden: false,
                config: cloud_config(),
                nix_packages: vec![],
            },
        ],
        mcp_bundles: vec![FederationMcpBundle {
            id: BUNDLE_ID,
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
