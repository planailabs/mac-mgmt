//! Fetch model lists from provider APIs and build grouped `ModelSource` trees.
//!
//! Used by both the `generate-model-catalog` CLI subcommand and the web UI's
//! static catalog loader. Gated behind `#[cfg(feature = "server")]` because it
//! depends on `reqwest`.

use mac_mgmt_common::model_source::{ModelEntry, ModelNode, ModelSource};
use std::collections::BTreeMap;

// ── Provider fetch functions ────────────────────────────────────────

/// Fetch models from the Ollama cloud library (ollama.com/api/tags).
pub async fn fetch_ollama_models(base_url: &str) -> Result<ModelSource, String> {
    #[derive(serde::Deserialize)]
    struct OllamaTagsResponse {
        #[serde(default)]
        models: Vec<OllamaTagModel>,
    }
    #[derive(serde::Deserialize)]
    struct OllamaTagModel {
        name: String,
        #[serde(default)]
        details: Option<OllamaDetails>,
    }
    #[derive(serde::Deserialize)]
    struct OllamaDetails {
        #[serde(default)]
        family: Option<String>,
        #[serde(default)]
        parameter_size: Option<String>,
    }

    let client = reqwest::Client::new();
    let resp: OllamaTagsResponse = client
        .get(format!("{base_url}/api/tags"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("ollama fetch: {e}"))?
        .json()
        .await
        .map_err(|e| format!("ollama parse: {e}"))?;

    let models: Vec<_> = resp
        .models
        .into_iter()
        .take(2000)
        .map(|m| {
            let family = m
                .details
                .as_ref()
                .and_then(|d| d.family.clone())
                .unwrap_or_default();
            let param_size = m
                .details
                .as_ref()
                .and_then(|d| d.parameter_size.clone())
                .unwrap_or_default();
            let display = if param_size.is_empty() {
                m.name.clone()
            } else {
                format!("{} ({})", m.name, param_size)
            };
            (
                ModelEntry {
                    model_id: m.name.clone(),
                    full_model_id: m.name,
                    display_name: display,
                },
                family,
            )
        })
        .collect();

    let family_map: std::collections::HashMap<String, String> = models
        .iter()
        .map(|(e, f)| (e.model_id.clone(), f.clone()))
        .collect();
    let entries: Vec<ModelEntry> = models.into_iter().map(|(e, _)| e).collect();

    let groups = auto_group(entries, |entry| {
        let family = family_map
            .get(&entry.model_id)
            .cloned()
            .unwrap_or_default();
        if family.is_empty() {
            vec![]
        } else {
            vec![family.to_lowercase()]
        }
    });

    Ok(ModelSource {
        id: "ollama".into(),
        display_name: "Ollama".into(),
        groups,
    })
}

/// Fetch models from OpenRouter (public, no auth required).
pub async fn fetch_openrouter_models() -> Result<ModelSource, String> {
    #[derive(serde::Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Vec<OpenRouterModel>,
    }
    #[derive(serde::Deserialize)]
    struct OpenRouterModel {
        id: String,
        #[serde(default)]
        name: String,
    }

    let client = reqwest::Client::new();
    let resp: ModelsResponse = client
        .get("https://openrouter.ai/api/v1/models")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("openrouter fetch: {e}"))?
        .json()
        .await
        .map_err(|e| format!("openrouter parse: {e}"))?;

    let entries: Vec<ModelEntry> = resp
        .data
        .into_iter()
        .map(|m| {
            let model_id = m.id.clone();
            let display = if m.name.is_empty() {
                model_id.clone()
            } else {
                m.name
            };
            ModelEntry {
                model_id: model_id.clone(),
                full_model_id: format!("openrouter/{model_id}"),
                display_name: display,
            }
        })
        .collect();

    let groups = auto_group(entries, |entry| {
        let parts: Vec<&str> = entry.model_id.splitn(2, '/').collect();
        if parts.len() == 2 {
            vec![parts[0].to_string()]
        } else {
            vec!["other".to_string()]
        }
    });

    Ok(ModelSource {
        id: "openrouter".into(),
        display_name: "OpenRouter".into(),
        groups,
    })
}

/// Fetch models from a cloud provider's /v1/models or equivalent.
///
/// Supported providers: `anthropic`, `google`, `openai`, `mistral`, `groq`,
/// `xai`, `deepseek`, `together`, and any other OpenAI-compatible API.
pub async fn fetch_cloud_provider_models(
    provider: &str,
    base_url: &str,
    api_key: &str,
) -> Result<ModelSource, String> {
    #[derive(serde::Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Option<Vec<ModelObj>>,
        #[serde(default)]
        models: Option<Vec<GoogleModelObj>>,
    }
    #[derive(serde::Deserialize)]
    struct ModelObj {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct GoogleModelObj {
        #[serde(default)]
        name: String,
        #[serde(default, rename = "displayName")]
        display_name: String,
    }

    let client = reqwest::Client::new();

    let entries = if provider == "google" {
        let url = format!("{base_url}/models?key={api_key}");
        let resp: ModelsResponse = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("{provider} fetch: {e}"))?
            .json()
            .await
            .map_err(|e| format!("{provider} parse: {e}"))?;
        let models = resp.models.unwrap_or_default();
        models
            .into_iter()
            .map(|m| {
                let id = m
                    .name
                    .strip_prefix("models/")
                    .unwrap_or(&m.name)
                    .to_string();
                let display = if m.display_name.is_empty() {
                    id.clone()
                } else {
                    m.display_name
                };
                ModelEntry {
                    model_id: id.clone(),
                    full_model_id: format!("{provider}/{id}"),
                    display_name: display,
                }
            })
            .collect::<Vec<_>>()
    } else if provider == "anthropic" {
        let url = format!("{base_url}/models");
        let resp: serde_json::Value = client
            .get(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("{provider} fetch: {e}"))?
            .json()
            .await
            .map_err(|e| format!("{provider} parse: {e}"))?;
        let data = resp
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        data.into_iter()
            .filter_map(|v| {
                let id = v.get("id")?.as_str()?.to_string();
                let display = v
                    .get("display_name")
                    .and_then(|d| d.as_str())
                    .unwrap_or(&id)
                    .to_string();
                Some(ModelEntry {
                    model_id: id.clone(),
                    full_model_id: format!("{provider}/{id}"),
                    display_name: display,
                })
            })
            .collect::<Vec<_>>()
    } else {
        // OpenAI-compatible: Mistral, Groq, xAI, Deepseek, Together, OpenAI, etc.
        let url = format!("{base_url}/models");
        let resp: ModelsResponse = client
            .get(&url)
            .bearer_auth(api_key)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("{provider} fetch: {e}"))?
            .json()
            .await
            .map_err(|e| format!("{provider} parse: {e}"))?;
        let models = resp.data.unwrap_or_default();
        models
            .into_iter()
            .map(|m| ModelEntry {
                model_id: m.id.clone(),
                full_model_id: format!("{provider}/{}", m.id),
                display_name: m.id,
            })
            .collect::<Vec<_>>()
    };

    let prov_display = titlecase(provider);
    let groups = auto_group(entries, |_| vec![]);

    Ok(ModelSource {
        id: provider.to_string(),
        display_name: prov_display,
        groups,
    })
}

// ── Grouping helpers ────────────────────────────────────────────────

pub fn titlecase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Split a model ID into semantic tokens for grouping.
/// Splits on `/`, `-`, and `:`, with adjacent numeric segments merged
/// into version numbers.
pub fn tokenize_model_id(model_id: &str) -> Vec<String> {
    let colon_parts: Vec<&str> = model_id.splitn(2, ':').collect();
    let base = colon_parts[0];
    let tag = colon_parts.get(1).copied();

    let raw_parts: Vec<&str> = base.split(&['/', '-'][..]).collect();

    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw_parts.len() {
        let part = raw_parts[i];
        if part.is_empty() {
            i += 1;
            continue;
        }
        if is_numeric_segment(part)
            && i + 1 < raw_parts.len()
            && is_numeric_segment(raw_parts[i + 1])
        {
            tokens.push(format!("{}.{}", part, raw_parts[i + 1]));
            i += 2;
        } else {
            tokens.push(part.to_string());
            i += 1;
        }
    }

    if let Some(t) = tag {
        tokens.push(format!(":{t}"));
    }

    tokens
}

fn is_numeric_segment(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Build recursive groups from a flat list of model entries.
/// `extra_prefix` returns additional group segments prepended to auto-detected tokens.
pub fn auto_group(
    entries: Vec<ModelEntry>,
    extra_prefix: impl Fn(&ModelEntry) -> Vec<String>,
) -> Vec<ModelNode> {
    let items: Vec<(Vec<String>, ModelEntry)> = entries
        .into_iter()
        .map(|e| {
            let mut segs = extra_prefix(&e);
            segs.extend(tokenize_model_id(&e.model_id));
            (segs, e)
        })
        .collect();

    build_groups_recursive(items, 0)
}

fn build_groups_recursive(
    items: Vec<(Vec<String>, ModelEntry)>,
    depth: usize,
) -> Vec<ModelNode> {
    if items.is_empty() {
        return vec![];
    }

    if items.len() == 1 || depth >= 6 {
        return items
            .into_iter()
            .map(|(_, e)| ModelNode::Model(e))
            .collect();
    }

    let mut buckets: BTreeMap<String, Vec<(Vec<String>, ModelEntry)>> = BTreeMap::new();
    let mut no_segment: Vec<(Vec<String>, ModelEntry)> = Vec::new();

    for item in items {
        if depth < item.0.len() {
            let key = item.0[depth].to_lowercase();
            buckets.entry(key).or_default().push(item);
        } else {
            no_segment.push(item);
        }
    }

    let mut result: Vec<ModelNode> = Vec::new();

    for (_, e) in no_segment {
        result.push(ModelNode::Model(e));
    }

    for (key, group) in buckets {
        if group.len() == 1 && key.starts_with(':') {
            result.push(ModelNode::Model(group.into_iter().next().unwrap().1));
        } else {
            let children = build_groups_recursive(group, depth + 1);
            if children.len() == 1 {
                if let ModelNode::Group {
                    name: child_name,
                    display_name: _,
                    children: grandchildren,
                } = &children[0]
                {
                    let merged_name = format!("{}-{}", key, child_name);
                    result.push(ModelNode::Group {
                        display_name: titlecase(&merged_name),
                        name: merged_name,
                        children: grandchildren.clone(),
                    });
                    continue;
                }
            }
            result.push(ModelNode::Group {
                display_name: titlecase(&key),
                name: key,
                children,
            });
        }
    }

    result.sort_by(|a, b| {
        let sort_key = |n: &ModelNode| match n {
            ModelNode::Group { name, .. } => (0, name.to_lowercase()),
            ModelNode::Model(e) => (1, e.model_id.to_lowercase()),
        };
        sort_key(a).cmp(&sort_key(b))
    });

    result
}

// ── Catalog types ───────────────────────────────────────────────────

/// The full model catalog written to `server/ext/model-catalog.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelCatalog {
    pub generated_at: String,
    pub sources: Vec<ModelSource>,
}
