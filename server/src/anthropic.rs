use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// ── Shared types (client + server) ──────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedNameDesc {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleItemContext {
    pub skill_slug: String,
    pub channel: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpBundleItemContext {
    pub server_slug: String,
    pub server_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenerateContext {
    Skill { skill_id: String },
    Bundle { slug: String, items: Vec<BundleItemContext> },
    McpServer { slug: String, config_json: String },
    McpBundle { slug: String, items: Vec<McpBundleItemContext> },
}

/// Entity kind for the bulk save server function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntityKind {
    Skill,
    Bundle,
    McpServer,
    McpBundle,
}

/// Item descriptor for GenerateAllButton.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateAllItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub context: GenerateContext,
    pub entity_kind: EntityKind,
}

// ── Server function ─────────────────────────────────────────────────

#[server]
pub async fn generate_name_desc(
    context: GenerateContext,
    current_name: String,
    current_desc: String,
) -> Result<GeneratedNameDesc, ServerFnError> {
    let cfg = crate::config::config();
    let anthropic = cfg.anthropic.as_ref()
        .ok_or_else(|| ServerFnError::new("Anthropic API key not configured"))?;

    let (system_msg, user_msg) = build_prompt(&context, &current_name, &current_desc).await?;

    let body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 300,
        "system": system_msg,
        "messages": [{"role": "user", "content": user_msg}],
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &anthropic.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ServerFnError::new(format!("Anthropic request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ServerFnError::new(format!("Anthropic API returned {status}: {text}")));
    }

    let api_resp: serde_json::Value = resp.json().await
        .map_err(|e| ServerFnError::new(format!("Failed to parse Anthropic response: {e}")))?;

    let text = api_resp["content"][0]["text"]
        .as_str()
        .ok_or_else(|| ServerFnError::new("No text in Anthropic response"))?;

    // Strip code fences if present
    let cleaned = text.trim();
    let json_str = if cleaned.starts_with("```") {
        let inner = cleaned
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        inner
    } else {
        cleaned
    };

    let result: GeneratedNameDesc = serde_json::from_str(json_str)
        .map_err(|e| ServerFnError::new(format!("Failed to parse generated JSON: {e}. Raw: {json_str}")))?;

    Ok(result)
}

/// Save a generated name+description for any entity type.
#[server]
pub async fn save_generated_name_desc(
    entity_kind: EntityKind,
    id: String,
    name: String,
    description: String,
) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let query = match entity_kind {
        EntityKind::Skill => "UPDATE skills SET name = $1, description = $2 WHERE id = $3",
        EntityKind::Bundle => "UPDATE bundles SET name = $1, description = $2 WHERE id = $3",
        EntityKind::McpServer => "UPDATE mcp_servers SET name = $1, description = $2 WHERE id = $3",
        EntityKind::McpBundle => {
            "UPDATE mcp_server_bundles SET name = $1, description = $2 WHERE id = $3"
        }
    };

    sqlx::query(query)
        .bind(&name)
        .bind(&description)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    match entity_kind {
        EntityKind::Bundle => {
            crate::api::push::notify_skill_bundle_global(uuid).await;
        }
        EntityKind::McpBundle => {
            crate::api::push::notify_mcp_bundle_global(uuid).await;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(feature = "server")]
async fn build_prompt(
    context: &GenerateContext,
    current_name: &str,
    current_desc: &str,
) -> Result<(String, String), ServerFnError> {
    let system = "You generate concise, human-friendly names and descriptions for IT management entities. \
        Output ONLY valid JSON: {\"name\": \"...\", \"description\": \"...\"}. \
        The name should be short (2-5 words). The description should be 1-2 sentences explaining what the entity does.".to_string();

    let user_msg = match context {
        GenerateContext::Skill { skill_id } => {
            let skill_md = read_skill_md(skill_id).await;
            let md_section = match &skill_md {
                Some(md) => format!("\n\nSKILL.md contents:\n{md}"),
                None => String::new(),
            };
            let pool = crate::server_pool()?;
            let uuid: uuid::Uuid = skill_id.parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            let slug = sqlx::query_scalar::<_, String>("SELECT slug FROM skills WHERE id = $1")
                .bind(uuid)
                .fetch_one(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            format!(
                "Generate a name and description for a Skill.\n\
                Slug: {slug}\n\
                Current name: {current_name}\n\
                Current description: {current_desc}{md_section}"
            )
        }
        GenerateContext::Bundle { slug, items } => {
            let items_str = if items.is_empty() {
                "No items yet.".to_string()
            } else {
                items.iter()
                    .map(|i| format!("  - {} / {}", i.skill_slug, i.channel))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            format!(
                "Generate a name and description for a Skill Bundle.\n\
                Slug: {slug}\n\
                Current name: {current_name}\n\
                Current description: {current_desc}\n\
                Contained skill channels:\n{items_str}"
            )
        }
        GenerateContext::McpServer { slug, config_json } => {
            let truncated = if config_json.len() > 4000 {
                &config_json[..4000]
            } else {
                config_json.as_str()
            };
            format!(
                "Generate a name and description for an MCP Server.\n\
                Slug: {slug}\n\
                Current name: {current_name}\n\
                Current description: {current_desc}\n\
                Config JSON:\n{truncated}"
            )
        }
        GenerateContext::McpBundle { slug, items } => {
            let items_str = if items.is_empty() {
                "No items yet.".to_string()
            } else {
                items.iter()
                    .map(|i| format!("  - {} ({})", i.server_name, i.server_slug))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            format!(
                "Generate a name and description for an MCP Server Bundle.\n\
                Slug: {slug}\n\
                Current name: {current_name}\n\
                Current description: {current_desc}\n\
                Contained MCP servers:\n{items_str}"
            )
        }
    };

    Ok((system, user_msg))
}

/// Read SKILL.md from the Nix store for a given skill ID.
#[cfg(feature = "server")]
async fn read_skill_md(skill_id: &str) -> Option<String> {
    let cfg = crate::config::config();
    let pool = crate::server_pool().ok()?;
    let uuid: uuid::Uuid = skill_id.parse().ok()?;

    let slug = sqlx::query_scalar::<_, String>("SELECT slug FROM skills WHERE id = $1")
        .bind(uuid)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten()?;

    let xzar = cfg.xzar.as_ref()?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .ok()?;

    let prefix = format!("skill/{slug}/");
    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        if pin.name.starts_with(&prefix) {
            if let Some(path) = crate::xzar::store_path_for_pin(&pins, &pin.name) {
                let skill_md_path = format!("{path}/SKILL.md");
                if let Ok(content) = tokio::fs::read_to_string(&skill_md_path).await {
                    let truncated = if content.len() > 4000 {
                        content[..4000].to_string()
                    } else {
                        content
                    };
                    return Some(truncated);
                }
            }
        }
    }

    None
}
