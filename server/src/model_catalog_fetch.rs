//! Fetch model lists from provider APIs and build grouped `ModelSource` trees.
//!
//! Used by both the `generate-model-catalog` CLI subcommand and the web UI's
//! static catalog loader. Gated behind `#[cfg(feature = "server")]` because it
//! depends on `reqwest`.

use mac_mgmt_common::model_source::{ModelEntry, ModelNode, ModelSource};
use std::collections::BTreeMap;

// ── Provider fetch functions ────────────────────────────────────────

/// Fetch models from the Ollama library by scraping the search pages.
///
/// Paginates through `{base_url}/search?page=N&o=newest` using HTMX headers
/// until no more "next page" link is found. A short delay is inserted between
/// requests to avoid overwhelming the server.
pub async fn fetch_ollama_models(base_url: &str) -> Result<ModelSource, String> {
    use scraper::{Html, Selector};

    let client = reqwest::Client::new();
    let model_sel = Selector::parse("li[x-test-model]").unwrap();
    let title_sel = Selector::parse("[x-test-search-response-title]").unwrap();
    let size_sel = Selector::parse("[x-test-size]").unwrap();
    let next_page_sel = Selector::parse("li[hx-get]").unwrap();
    let link_sel = Selector::parse("a[href]").unwrap();
    // Capabilities: vision, tools, thinking, embedding, cloud, etc.
    let cap_sel = Selector::parse("[x-test-capability]").unwrap();

    let mut entries: Vec<ModelEntry> = Vec::new();
    let mut page = 1u32;
    let delay = std::time::Duration::from_millis(500);

    loop {
        let url = format!("{base_url}/search?page={page}&o=newest");
        tracing::info!("ollama: fetching page {page} ({url})");

        let resp = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:149.0) Gecko/20100101 Firefox/149.0")
            .header("HX-Request", "true")
            .header("HX-Current-URL", format!("{base_url}/search?o=newest"))
            .header("Referer", format!("{base_url}/search?o=newest"))
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("ollama page {page} fetch: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("ollama page {page}: HTTP {}", resp.status()));
        }

        let html_text = resp
            .text()
            .await
            .map_err(|e| format!("ollama page {page} read: {e}"))?;

        let doc = Html::parse_fragment(&html_text);
        let mut page_count = 0u32;

        for li in doc.select(&model_sel) {
            // Extract model name from the title span
            let name = li
                .select(&title_sel)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            // Extract sizes from x-test-size spans (e.g. "128b", "70b")
            let sizes: Vec<String> = li
                .select(&size_sel)
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Extract capabilities
            let caps: Vec<String> = li
                .select(&cap_sel)
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Check capabilities
            let has_tools = caps.iter().any(|c| c.eq_ignore_ascii_case("tools"));
            let is_cloud = caps.iter().any(|c| c.eq_ignore_ascii_case("cloud"));

            // Extract the href to get the canonical model path (e.g. "/library/mistral-medium-3.5")
            let href = li
                .select(&link_sel)
                .next()
                .and_then(|a| a.value().attr("href"))
                .unwrap_or_default();
            let model_id = href
                .strip_prefix("/library/")
                .unwrap_or(&name)
                .to_string();

            tracing::debug!("  {model_id} sizes={sizes:?} caps={caps:?} tools={has_tools} cloud={is_cloud}");

            // Emit individual `:size` entries for each advertised size,
            // plus a base entry when there are no sizes or as the default.
            if sizes.is_empty() {
                entries.push(ModelEntry {
                    model_id: model_id.clone(),
                    full_model_id: model_id,
                    display_name: name,
                    supports_tools: has_tools,
                    sizes: vec![],
                    is_cloud,
                });
            } else {
                for size in &sizes {
                    let sized_id = format!("{model_id}:{size}");
                    let sized_display = format!("{name}:{size}");
                    entries.push(ModelEntry {
                        model_id: sized_id.clone(),
                        full_model_id: sized_id,
                        display_name: sized_display,
                        supports_tools: has_tools,
                        sizes: sizes.clone(),
                        is_cloud,
                    });
                }
            }
            page_count += 1;
        }

        tracing::info!("ollama: page {page} yielded {page_count} models (total: {})", entries.len());

        // Check for next page: look for an <li> with hx-get="/search?page=N"
        let has_next = doc.select(&next_page_sel).any(|el| {
            el.value()
                .attr("hx-get")
                .map(|v| v.contains("page="))
                .unwrap_or(false)
        });

        if !has_next || page_count == 0 {
            tracing::info!("ollama: no more pages after page {page}");
            break;
        }

        page += 1;
        // Rate-limit: wait before fetching the next page
        tokio::time::sleep(delay).await;
    }

    tracing::info!("ollama: scraped {} models total across {page} page(s)", entries.len());

    let groups = auto_group(entries, |_| vec![]);

    Ok(ModelSource {
        id: "ollama".into(),
        display_name: "Ollama".into(),
        groups,
    })
}

/// Fetch models from the LM Studio catalog (lmstudio.ai/models).
///
/// 1. Scrapes the listing page for model slugs (`/models/<slug>`)
/// 2. Fetches each model's page and extracts the JSON-LD `CreativeWork`
///    block which contains the display name and model IDs in `keywords`.
pub async fn fetch_lms_models(base_url: &str) -> Result<ModelSource, String> {
    use scraper::{Html, Selector};

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:149.0) Gecko/20100101 Firefox/149.0")
        .build()
        .map_err(|e| format!("lms: client build: {e}"))?;

    // Step 1: Get all model slugs from the listing page
    let listing_url = format!("{base_url}/models?sort=created");
    tracing::info!("lms: fetching listing {listing_url}");

    let resp = client
        .get(&listing_url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("lms listing fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("lms listing: HTTP {}", resp.status()));
    }
    let html_text = resp
        .text()
        .await
        .map_err(|e| format!("lms listing read: {e}"))?;

    let doc = Html::parse_document(&html_text);
    let link_sel = Selector::parse("a[href]").unwrap();

    let mut slugs: Vec<String> = Vec::new();
    for el in doc.select(&link_sel) {
        if let Some(href) = el.value().attr("href") {
            if let Some(slug) = href.strip_prefix("/models/") {
                if !slug.is_empty() && !slug.contains('?') && !slug.contains('/') {
                    let s = slug.to_string();
                    if !slugs.contains(&s) {
                        slugs.push(s);
                    }
                }
            }
        }
    }

    tracing::info!("lms: found {} model slugs", slugs.len());

    // Step 2: Fetch each model page and extract from JSON-LD
    let mut entries: Vec<ModelEntry> = Vec::new();
    let delay = std::time::Duration::from_millis(300);
    let ld_sel = Selector::parse("script[type='application/ld+json']").unwrap();

    for (i, slug) in slugs.iter().enumerate() {
        let model_url = format!("{base_url}/models/{slug}");
        tracing::info!("lms: [{}/{}] fetching {model_url}", i + 1, slugs.len());

        let resp = match client
            .get(&model_url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("lms: {slug}: fetch error: {e}");
                continue;
            }
        };
        if !resp.status().is_success() {
            tracing::warn!("lms: {slug}: HTTP {}", resp.status());
            continue;
        }
        let page_html = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("lms: {slug}: read error: {e}");
                continue;
            }
        };

        let page_doc = Html::parse_document(&page_html);

        // Find the CreativeWork JSON-LD block
        let mut display_name = String::new();
        let mut model_ids: Vec<String> = Vec::new();

        for script in page_doc.select(&ld_sel) {
            let json_text = script.text().collect::<String>();
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_text) {
                if data.get("@type").and_then(|t| t.as_str()) == Some("CreativeWork") {
                    display_name = data
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or(slug)
                        .to_string();

                    if let Some(keywords) = data.get("keywords").and_then(|k| k.as_array()) {
                        for kw in keywords {
                            if let Some(s) = kw.as_str() {
                                // Model IDs contain a `/` and are lowercase
                                // e.g. "qwen/qwen3-4b-2507", skip display patterns like "qwen/Qwen3"
                                if s.contains('/') && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '/' | '-' | '_' | '.'))
                                {
                                    model_ids.push(s.to_string());
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }

        if model_ids.is_empty() {
            tracing::debug!("lms: {slug}: no model IDs found, using slug");
            entries.push(ModelEntry {
                model_id: slug.clone(),
                full_model_id: slug.clone(),
                display_name: if display_name.is_empty() {
                    slug.clone()
                } else {
                    display_name
                },
                ..Default::default()
            });
        } else {
            tracing::debug!("lms: {slug}: {} variant(s): {:?}", model_ids.len(), model_ids);
            for mid in &model_ids {
                let short_id = mid.split('/').last().unwrap_or(mid).to_string();
                let variant_display = if model_ids.len() == 1 {
                    display_name.clone()
                } else {
                    format!("{display_name} ({short_id})")
                };
                entries.push(ModelEntry {
                    model_id: mid.clone(),
                    full_model_id: mid.clone(),
                    display_name: variant_display,
                    ..Default::default()
                });
            }
        }

        // Rate-limit
        if i + 1 < slugs.len() {
            tokio::time::sleep(delay).await;
        }
    }

    tracing::info!("lms: scraped {} models from {} pages", entries.len(), slugs.len());

    let groups = auto_group(entries, |entry| {
        // Group by org prefix (e.g. "qwen", "google", "nvidia")
        let provider = entry
            .model_id
            .split('/')
            .next()
            .unwrap_or("other")
            .to_string();
        if provider == entry.model_id {
            vec![] // no slash, no provider prefix
        } else {
            vec![provider]
        }
    });

    Ok(ModelSource {
        id: "lms".into(),
        display_name: "LM Studio".into(),
        groups,
    })
}

/// Fetch models from an OpenClaw gateway (`/v1/models`).
pub async fn fetch_openclaw_models(
    host: &str,
    port: u16,
    token: Option<&str>,
) -> Result<ModelSource, String> {
    #[derive(serde::Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Vec<ModelObj>,
    }
    #[derive(serde::Deserialize)]
    struct ModelObj {
        id: String,
        #[serde(default)]
        owned_by: String,
    }

    let url = format!("http://{host}:{port}/v1/models");
    tracing::info!("openclaw: fetching {url}");

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10));
    if let Some(tok) = token {
        req = req.bearer_auth(tok);
    }
    let resp: ModelsResponse = req
        .send()
        .await
        .map_err(|e| format!("openclaw fetch: {e}"))?
        .json()
        .await
        .map_err(|e| format!("openclaw parse: {e}"))?;

    tracing::info!("openclaw: received {} raw models", resp.data.len());

    let entries: Vec<ModelEntry> = resp
        .data
        .into_iter()
        .map(|m| {
            let provider = if !m.owned_by.is_empty() {
                m.owned_by.clone()
            } else {
                m.id.split('/').next().unwrap_or("unknown").to_string()
            };
            let model_id = if m.id.contains('/') {
                m.id.split('/').last().unwrap_or(&m.id).to_string()
            } else {
                m.id.clone()
            };
            ModelEntry {
                display_name: model_id.clone(),
                model_id,
                full_model_id: if m.id.contains('/') {
                    m.id
                } else {
                    format!("{provider}/{}", m.id)
                },
                ..Default::default()
            }
        })
        .collect();

    let groups = auto_group(entries, |entry| {
        let provider = entry
            .full_model_id
            .split('/')
            .next()
            .unwrap_or("unknown")
            .to_string();
        vec![provider]
    });

    Ok(ModelSource {
        id: "openclaw".into(),
        display_name: "OpenClaw".into(),
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
                ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                ..Default::default()
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
