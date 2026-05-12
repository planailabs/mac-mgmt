use dioxus::prelude::*;
use dioxus_i18n::t;
use mac_mgmt_common::model_source::{ModelEntry, ModelNode, ModelSource};
use std::collections::HashSet;

// ── Server functions ─────────────────────────────────────────────────

/// Fetch available models from the Ollama cloud library (ollama.com/api/tags).
#[server]
pub async fn fetch_ollama_models() -> Result<ModelSource, ServerFnError> {
    fetch_ollama_models_inner("https://ollama.com").await
}

#[cfg(feature = "server")]
async fn fetch_ollama_models_inner(base_url: &str) -> Result<ModelSource, ServerFnError> {
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
        .map_err(|e| ServerFnError::new(format!("ollama fetch: {e}")))?
        .json()
        .await
        .map_err(|e| ServerFnError::new(format!("ollama parse: {e}")))?;

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

    // Build a family lookup from the API response's `details.family` field.
    let family_map: std::collections::HashMap<String, String> = models
        .iter()
        .map(|(e, f)| (e.model_id.clone(), f.clone()))
        .collect();
    let entries: Vec<ModelEntry> = models.into_iter().map(|(e, _)| e).collect();

    // Group by family (from API), then recursively by model ID tokens.
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

/// Fetch models from the OpenClaw gateway.
#[server]
pub async fn fetch_openclaw_models(
    gateway_host: String,
    gateway_port: u16,
    token: Option<String>,
) -> Result<ModelSource, ServerFnError> {
    fetch_openclaw_models_inner(&gateway_host, gateway_port, token.as_deref()).await
}

#[cfg(feature = "server")]
async fn fetch_openclaw_models_inner(
    host: &str,
    port: u16,
    token: Option<&str>,
) -> Result<ModelSource, ServerFnError> {
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
        .map_err(|e| ServerFnError::new(format!("openclaw fetch: {e}")))?
        .json()
        .await
        .map_err(|e| ServerFnError::new(format!("openclaw parse: {e}")))?;

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
#[server]
pub async fn fetch_openrouter_models() -> Result<ModelSource, ServerFnError> {
    fetch_openrouter_models_inner().await
}

#[cfg(feature = "server")]
async fn fetch_openrouter_models_inner() -> Result<ModelSource, ServerFnError> {
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
        .map_err(|e| ServerFnError::new(format!("openrouter fetch: {e}")))?
        .json()
        .await
        .map_err(|e| ServerFnError::new(format!("openrouter parse: {e}")))?;

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
        // OpenRouter IDs are like "anthropic/claude-sonnet-4-6"
        // Extract the provider prefix as the first group segment.
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
#[server]
pub async fn fetch_cloud_provider_models(
    provider: String,
    base_url: String,
    api_key: String,
) -> Result<ModelSource, ServerFnError> {
    fetch_cloud_provider_models_inner(&provider, &base_url, &api_key).await
}

#[cfg(feature = "server")]
async fn fetch_cloud_provider_models_inner(
    provider: &str,
    base_url: &str,
    api_key: &str,
) -> Result<ModelSource, ServerFnError> {
    #[derive(serde::Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Option<Vec<ModelObj>>,
        // Google uses { models: [...] } instead of { data: [...] }
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

    let (url, entries) = if provider == "google" {
        let url = format!("{base_url}/models?key={api_key}");
        let resp: ModelsResponse = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ServerFnError::new(format!("{provider} fetch: {e}")))?
            .json()
            .await
            .map_err(|e| ServerFnError::new(format!("{provider} parse: {e}")))?;
        let models = resp.models.unwrap_or_default();
        let entries: Vec<ModelEntry> = models
            .into_iter()
            .map(|m| {
                // Google model name is like "models/gemini-2.5-flash"
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
            .collect();
        (url, entries)
    } else if provider == "anthropic" {
        let url = format!("{base_url}/models");
        let resp: serde_json::Value = client
            .get(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ServerFnError::new(format!("{provider} fetch: {e}")))?
            .json()
            .await
            .map_err(|e| ServerFnError::new(format!("{provider} parse: {e}")))?;
        let data = resp
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let entries: Vec<ModelEntry> = data
            .into_iter()
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
            .collect();
        (url, entries)
    } else {
        // OpenAI-compatible: Mistral, Groq, xAI, Deepseek, Together, OpenAI, etc.
        let url = format!("{base_url}/models");
        let resp: ModelsResponse = client
            .get(&url)
            .bearer_auth(api_key)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ServerFnError::new(format!("{provider} fetch: {e}")))?
            .json()
            .await
            .map_err(|e| ServerFnError::new(format!("{provider} parse: {e}")))?;
        let models = resp.data.unwrap_or_default();
        let entries: Vec<ModelEntry> = models
            .into_iter()
            .map(|m| ModelEntry {
                model_id: m.id.clone(),
                full_model_id: format!("{provider}/{}", m.id),
                display_name: m.id,
            })
            .collect();
        (url, entries)
    };

    let _ = url; // suppress unused

    let prov_display = titlecase(provider);
    // Recursive grouping by model ID tokens — no hardcoded families needed.
    let groups = auto_group(entries, |_| vec![]);

    Ok(ModelSource {
        id: provider.to_string(),
        display_name: prov_display,
        groups,
    })
}

// ── Grouping helpers ─────────────────────────────────────────────────

#[cfg(feature = "server")]
fn titlecase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Split a model ID into semantic tokens for grouping.
/// Splits on `/`, `-`, and `:`, with adjacent numeric segments merged
/// into version numbers.
///
/// "claude-sonnet-4-6"                → ["claude", "sonnet", "4.6"]
/// "gpt-5.4-mini"                     → ["gpt", "5.4", "mini"]
/// "gemini-2.5-flash"                 → ["gemini", "2.5", "flash"]
/// "qwen3:0.6b"                       → ["qwen3", ":0.6b"]
/// "llama3.3:70b"                     → ["llama3.3", ":70b"]
/// "anthropic/claude-sonnet-4-6"      → ["anthropic", "claude", "sonnet", "4.6"]
/// "meta-llama/llama-3.3-70b"         → ["meta", "llama", "llama", "3.3", "70b"]
#[cfg(feature = "server")]
fn tokenize_model_id(model_id: &str) -> Vec<String> {
    // Split on `:` — first part is the base name, rest are tags (size variants).
    let colon_parts: Vec<&str> = model_id.splitn(2, ':').collect();
    let base = colon_parts[0];
    let tag = colon_parts.get(1).copied();

    // Split on both `/` and `-` to handle "provider/model-name" and plain
    // "model-name" uniformly.
    let raw_parts: Vec<&str> = base.split(&['/', '-'][..]).collect();

    // Merge adjacent purely-numeric parts into version numbers:
    // e.g. ["4", "6"] → "4.6"; standalone "3.5" stays as-is.
    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw_parts.len() {
        let part = raw_parts[i];
        if part.is_empty() {
            i += 1;
            continue;
        }
        if is_numeric_segment(part) && i + 1 < raw_parts.len() && is_numeric_segment(raw_parts[i + 1]) {
            tokens.push(format!("{}.{}", part, raw_parts[i + 1]));
            i += 2;
        } else {
            tokens.push(part.to_string());
            i += 1;
        }
    }

    // Add the `:tag` as a separate grouping level so size variants cluster
    // under the base model name: e.g. "qwen3" group contains ":0.6b", ":4b", ":8b".
    if let Some(t) = tag {
        tokens.push(format!(":{t}"));
    }

    tokens
}

#[cfg(feature = "server")]
fn is_numeric_segment(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Build recursive groups from a flat list of (model_entry, extra_segments) pairs.
/// `extra_segments` are prepended to the auto-detected token groups (e.g. provider name).
///
/// Algorithm: given a set of model IDs, find the longest shared prefix tokens among
/// siblings. If ≥2 models share a prefix, group them under that prefix. Recurse
/// into each group with the remaining suffix tokens.
#[cfg(feature = "server")]
fn auto_group(entries: Vec<ModelEntry>, extra_prefix: impl Fn(&ModelEntry) -> Vec<String>) -> Vec<ModelNode> {
    // Build (full_segments, entry) pairs.
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

#[cfg(feature = "server")]
fn build_groups_recursive(items: Vec<(Vec<String>, ModelEntry)>, depth: usize) -> Vec<ModelNode> {
    use std::collections::BTreeMap;

    if items.is_empty() {
        return vec![];
    }

    // If only one item or we've exhausted segments, emit leaves.
    if items.len() == 1 || depth >= 6 {
        return items
            .into_iter()
            .map(|(_, e)| ModelNode::Model(e))
            .collect();
    }

    // Group by the token at `depth`.
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

    // Items that ran out of segments become leaves.
    for (_, e) in no_segment {
        result.push(ModelNode::Model(e));
    }

    for (key, group) in buckets {
        if group.len() == 1 && key.starts_with(':') {
            // Single model with a `:tag` — emit as leaf, don't wrap in a group.
            result.push(ModelNode::Model(group.into_iter().next().unwrap().1));
        } else {
            let children = build_groups_recursive(group, depth + 1);
            // If recursion produced a single group child, unwrap it
            // to avoid unnecessary nesting like "X" → "Y" → items.
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

    // Sort: groups first (alphabetically), then models.
    result.sort_by(|a, b| {
        let sort_key = |n: &ModelNode| match n {
            ModelNode::Group { name, .. } => (0, name.to_lowercase()),
            ModelNode::Model(e) => (1, e.model_id.to_lowercase()),
        };
        sort_key(a).cmp(&sort_key(b))
    });

    result
}

// ── Modal state ──────────────────────────────────────────────────────

/// Request to open the model selection modal. Set into a context signal.
#[derive(Clone, Debug)]
pub struct ModelSelectRequest {
    pub source_kind: String,
    pub current: Vec<String>,
    pub multi: bool,
    pub field_path: Vec<String>,
    /// For cloud: provider slug
    pub provider: Option<String>,
    /// For cloud: base URL
    pub base_url: Option<String>,
    /// For cloud: API key (from the form being edited)
    pub api_key: Option<String>,
    /// For openclaw: gateway host
    pub gateway_host: Option<String>,
    /// For openclaw: gateway port
    pub gateway_port: Option<u16>,
}

// ── Modal component ──────────────────────────────────────────────────

#[component]
pub fn ModelSelectModal(
    request: Signal<Option<ModelSelectRequest>>,
    form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
) -> Element {
    // Close & reset when request is None
    let Some(req) = request.read().clone() else {
        return rsx! {};
    };

    let mut selected: Signal<HashSet<String>> = use_signal(|| {
        req.current.iter().cloned().collect::<HashSet<_>>()
    });
    let mut custom_models: Signal<Vec<String>> = use_signal(Vec::new);
    let mut custom_input: Signal<String> = use_signal(String::new);
    let mut filter: Signal<String> = use_signal(String::new);
    let mut show_selected: Signal<bool> = use_signal(|| false);
    let mut expanded: Signal<HashSet<String>> = use_signal(HashSet::new);

    // Build an identity key from the request so we can detect when a
    // different field opens the modal and reset all internal state.
    let request_key = format!("{}:{}", req.field_path.join("."), req.source_kind);
    let mut prev_key: Signal<String> = use_signal(String::new);
    if *prev_key.read() != request_key {
        prev_key.set(request_key);
        selected.set(req.current.iter().cloned().collect());
        custom_models.set(Vec::new());
        filter.set(String::new());
        show_selected.set(false);
        expanded.set(HashSet::new());
    }

    // Fetch model source.
    let source_kind = req.source_kind.clone();
    let provider = req.provider.clone();
    let base_url = req.base_url.clone();
    let api_key = req.api_key.clone();
    let gw_host = req.gateway_host.clone();
    let gw_port = req.gateway_port;

    let catalog = use_server_future(move || {
        let sk = source_kind.clone();
        let prov = provider.clone();
        let bu = base_url.clone();
        let ak = api_key.clone();
        let gh = gw_host.clone();
        let gp = gw_port;
        async move {
            match sk.as_str() {
                "ollama" => fetch_ollama_models().await,
                "openrouter" => fetch_openrouter_models().await,
                "openclaw" => {
                    let host = gh.unwrap_or_else(|| "127.0.0.1".into());
                    let port = gp.unwrap_or(18789);
                    fetch_openclaw_models(host, port, None).await
                }
                "cloud" => {
                    if let (Some(p), Some(b), Some(k)) = (prov, bu, ak) {
                        fetch_cloud_provider_models(p, b, k).await
                    } else {
                        Ok(ModelSource {
                            id: "cloud".into(),
                            display_name: "Cloud".into(),
                            groups: vec![],
                        })
                    }
                }
                // "lms" uses ollama catalog too
                "lms" => fetch_ollama_models().await,
                // "custom-only" — no catalog
                _ => Ok(ModelSource {
                    id: sk.clone(),
                    display_name: sk,
                    groups: vec![],
                }),
            }
        }
    })?;

    // Determine if catalog is loading / loaded / errored.
    let (catalog_source, loading, error_msg) = match &*catalog.read() {
        Some(Ok(src)) => (Some(src.clone()), false, None),
        Some(Err(e)) => (None, false, Some(e.to_string())),
        None => (None, true, None),
    };

    // Apply search filter to the catalog tree.
    let filter_str = filter.read().clone();
    let show_sel = *show_selected.read();

    let filtered_tree: Vec<ModelNode> = if let Some(ref src) = catalog_source {
        let tree = if filter_str.is_empty() {
            src.groups.clone()
        } else {
            src.search(&filter_str)
        };
        if show_sel {
            let sel = selected.read();
            tree.into_iter()
                .filter_map(|n| filter_to_selected(&n, &sel))
                .collect()
        } else {
            tree
        }
    } else {
        vec![]
    };

    // Identify custom models: entries in `current` that are NOT in the catalog.
    let catalog_ids: HashSet<String> = catalog_source
        .as_ref()
        .map(|s| s.list_all().into_iter().map(|e| e.full_model_id).collect())
        .unwrap_or_default();
    // On first load, populate custom_models from current selections not in catalog.
    use_effect(move || {
        if !loading {
            let current_customs: Vec<String> = req
                .current
                .iter()
                .filter(|id| !catalog_ids.contains(*id))
                .cloned()
                .collect();
            if !current_customs.is_empty() {
                custom_models.set(current_customs);
            }
        }
    });

    let multi = req.multi;
    let field_path = req.field_path.clone();

    let close = move |_: MouseEvent| {
        request.set(None);
    };

    let apply = {
        let fp = field_path.clone();
        move |evt: MouseEvent| {
            evt.prevent_default();
            evt.stop_propagation();
            let sel = selected.read().clone();
            if multi {
                let arr: Vec<serde_json::Value> =
                    sel.into_iter().map(serde_json::Value::String).collect();
                set_at_path(&mut form_values, &fp, serde_json::Value::Array(arr));
            } else if let Some(val) = sel.into_iter().next() {
                set_at_path(&mut form_values, &fp, serde_json::Value::String(val));
            }
            // Sync JSON text
            json_text.set(serde_json::to_string_pretty(&*form_values.read()).unwrap_or_default());
            request.set(None);
        }
    };

    let sel_count = selected.read().len();

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50",
            onclick: close,
            div {
                class: "bg-surface rounded-lg shadow-xl max-w-3xl w-full max-h-[85vh] flex flex-col",
                onclick: move |e| e.stop_propagation(),

                // ── Header ───────────────────────────────────────
                div { class: "px-4 py-3 border-b border-line-soft flex items-center justify-between",
                    h2 { class: "font-semibold text-base text-fg-strong", {t!("model-select-title")} }
                    button {
                        r#type: "button",
                        class: "text-fg-muted hover:text-fg-strong text-xl leading-none",
                        onclick: move |evt| {
                            evt.prevent_default();
                            evt.stop_propagation();
                            request.set(None);
                        },
                        "×"
                    }
                }

                // ── Search + toggles ─────────────────────────────
                div { class: "px-4 py-2 border-b border-line-soft space-y-2",
                    // Search bar with clear button
                    div { class: "relative",
                        input {
                            r#type: "text",
                            class: "input input-sm w-full pr-8",
                            placeholder: t!("model-select-search"),
                            value: "{filter}",
                            oninput: move |e| filter.set(e.value()),
                        }
                        if !filter.read().is_empty() {
                            button {
                                r#type: "button",
                                class: "absolute right-2 top-1/2 -translate-y-1/2 w-5 h-5 flex items-center justify-center rounded-full bg-surface-3 text-fg-muted hover:text-fg-strong text-xs leading-none",
                                onclick: move |_| filter.set(String::new()),
                                "×"
                            }
                        }
                    }
                    // Show selected toggle
                    label { class: "flex items-center gap-2 text-sm text-fg-muted cursor-pointer select-none",
                        input {
                            r#type: "checkbox",
                            class: "accent-brand",
                            checked: *show_selected.read(),
                            onchange: move |e| show_selected.set(e.checked()),
                        }
                        {t!("model-select-show-selected")}
                    }
                }

                // ── Content ──────────────────────────────────────
                div { class: "px-4 py-3 overflow-y-auto flex-1 space-y-3",
                    // Custom models section
                    div { class: "space-y-1",
                        div { class: "text-xs font-semibold text-fg-muted uppercase tracking-wider",
                            {t!("model-select-custom")}
                        }
                        // List custom models
                        for (_idx, cm) in custom_models.read().iter().enumerate() {
                            {
                                let cm_id = cm.clone();
                                let cm_display = cm.clone();
                                rsx! {
                                    div { class: "flex items-center gap-2 py-1 pl-2",
                                        input {
                                            r#type: if multi { "checkbox" } else { "radio" },
                                            class: "accent-brand",
                                            checked: selected.read().contains(&cm_id),
                                            onchange: {
                                                let id = cm_id.clone();
                                                move |_| toggle_selection(selected, &id, multi)
                                            },
                                        }
                                        span { class: "text-sm font-mono", "{cm_display}" }
                                        button {
                                            r#type: "button",
                                            class: "ml-auto text-fg-muted hover:text-danger text-xs",
                                            onclick: {
                                                let id = cm_id.clone();
                                                move |_| {
                                                    custom_models.write().retain(|x| x != &id);
                                                    selected.write().remove(&id);
                                                }
                                            },
                                            "✕"
                                        }
                                    }
                                }
                            }
                        }
                        // Add custom model input
                        div { class: "flex items-center gap-2",
                            input {
                                r#type: "text",
                                class: "input input-sm flex-1",
                                placeholder: t!("model-select-custom-placeholder"),
                                value: "{custom_input}",
                                oninput: move |e| custom_input.set(e.value()),
                                onkeypress: move |e| {
                                    if e.key() == Key::Enter {
                                        add_custom_model(custom_input, custom_models, selected, multi);
                                    }
                                },
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-sm btn-secondary",
                                disabled: custom_input.read().trim().is_empty(),
                                onclick: move |_| {
                                    add_custom_model(custom_input, custom_models, selected, multi);
                                },
                                "+ Add"
                            }
                        }
                    }

                    // Catalog section
                    if loading {
                        div { class: "flex items-center gap-2 text-sm text-fg-muted py-4",
                            div { class: "animate-spin w-4 h-4 border-2 border-brand border-t-transparent rounded-full" }
                            {t!("model-select-loading")}
                        }
                    } else if let Some(err) = &error_msg {
                        div { class: "text-sm text-danger py-2",
                            {t!("model-select-error", error: err.clone())}
                        }
                    } else if !filtered_tree.is_empty() {
                        div { class: "space-y-0",
                            div { class: "text-xs font-semibold text-fg-muted uppercase tracking-wider mb-1",
                                {t!("model-select-catalog")}
                            }
                            for node in &filtered_tree {
                                {render_model_node(node, selected, expanded, 0, multi, "")}
                            }
                        }
                    }
                }

                // ── Footer ───────────────────────────────────────
                div { class: "px-4 py-3 border-t border-line-soft flex justify-end gap-2",
                    button {
                        r#type: "button",
                        class: "btn btn-md btn-secondary",
                        onclick: move |evt| {
                            evt.prevent_default();
                            evt.stop_propagation();
                            request.set(None);
                        },
                        {t!("cancel")}
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-md btn-primary",
                        onclick: apply,
                        if sel_count > 0 {
                            {t!("model-select-apply-count", count: sel_count)}
                        } else {
                            {t!("model-select-apply")}
                        }
                    }
                }
            }
        }
    }
}

// ── Helper functions ─────────────────────────────────────────────────

fn toggle_selection(mut selected: Signal<HashSet<String>>, id: &str, multi: bool) {
    let mut sel = selected.write();
    if sel.contains(id) {
        sel.remove(id);
    } else {
        if !multi {
            sel.clear();
        }
        sel.insert(id.to_string());
    }
}

fn add_custom_model(
    mut input: Signal<String>,
    mut custom_models: Signal<Vec<String>>,
    mut selected: Signal<HashSet<String>>,
    multi: bool,
) {
    let val = input.read().trim().to_string();
    if val.is_empty() {
        return;
    }
    if !custom_models.read().contains(&val) {
        custom_models.write().push(val.clone());
    }
    if !multi {
        selected.write().clear();
    }
    selected.write().insert(val);
    input.set(String::new());
}

fn filter_to_selected(node: &ModelNode, selected: &HashSet<String>) -> Option<ModelNode> {
    match node {
        ModelNode::Model(entry) => {
            if selected.contains(&entry.full_model_id) {
                Some(node.clone())
            } else {
                None
            }
        }
        ModelNode::Group {
            name,
            display_name,
            children,
        } => {
            let filtered: Vec<_> = children
                .iter()
                .filter_map(|c| filter_to_selected(c, selected))
                .collect();
            if filtered.is_empty() {
                None
            } else {
                Some(ModelNode::Group {
                    name: name.clone(),
                    display_name: display_name.clone(),
                    children: filtered,
                })
            }
        }
    }
}

fn render_model_node(
    node: &ModelNode,
    mut selected: Signal<HashSet<String>>,
    mut expanded: Signal<HashSet<String>>,
    depth: usize,
    multi: bool,
    parent_path: &str,
) -> Element {
    let indent = format!("{}rem", depth as f32 * 1.0);
    match node {
        ModelNode::Group {
            name,
            display_name,
            children,
        } => {
            let path = if parent_path.is_empty() {
                name.clone()
            } else {
                format!("{parent_path}.{name}")
            };
            let is_expanded = expanded.read().contains(&path);
            let count = node.count_models();
            let toggle_path = path.clone();

            rsx! {
                div {
                    // Group header row
                    div {
                        class: "flex items-center gap-2 py-1 cursor-pointer hover:bg-surface-2 rounded px-1",
                        style: "padding-left: {indent}",
                        onclick: move |_| {
                            let mut exp = expanded.write();
                            if exp.contains(&toggle_path) {
                                exp.remove(&toggle_path);
                            } else {
                                exp.insert(toggle_path.clone());
                            }
                        },
                        span {
                            class: if is_expanded {
                                "text-fg-faint text-[10px] font-mono transition-transform rotate-90"
                            } else {
                                "text-fg-faint text-[10px] font-mono transition-transform"
                            },
                            "▶"
                        }
                        span { class: "text-sm font-semibold text-fg-strong", "{display_name}" }
                        span { class: "text-xs text-fg-muted", "({count})" }
                    }
                    // Children (if expanded)
                    if is_expanded {
                        for child in children {
                            {render_model_node(child, selected, expanded, depth + 1, multi, &path)}
                        }
                    }
                }
            }
        }
        ModelNode::Model(entry) => {
            let id = entry.full_model_id.clone();
            let is_checked = selected.read().contains(&id);

            rsx! {
                div {
                    class: "flex items-center gap-2 py-1 hover:bg-surface-2 rounded px-1",
                    style: "padding-left: {indent}",
                    input {
                        r#type: if multi { "checkbox" } else { "radio" },
                        class: "accent-brand",
                        checked: is_checked,
                        onchange: {
                            let id = id.clone();
                            move |_| {
                                let mut sel = selected.write();
                                if sel.contains(&id) {
                                    sel.remove(&id);
                                } else {
                                    if !multi {
                                        sel.clear();
                                    }
                                    sel.insert(id.clone());
                                }
                            }
                        },
                    }
                    span { class: "text-sm", "{entry.display_name}" }
                    if entry.display_name != entry.model_id {
                        span { class: "text-xs text-fg-muted font-mono", "{entry.model_id}" }
                    }
                }
            }
        }
    }
}

/// Set a value at a JSON path in form_values.
fn set_at_path(
    form_values: &mut Signal<serde_json::Value>,
    path: &[String],
    value: serde_json::Value,
) {
    use serde_json::Value;

    let mut root = form_values.write();
    // Navigate to the parent, then insert the last segment.
    if path.is_empty() {
        return;
    }
    let (parents, last) = path.split_at(path.len() - 1);
    let last_key = &last[0];

    let mut current = &mut *root;
    for segment in parents {
        if let Ok(idx) = segment.parse::<usize>() {
            if !current.is_array() {
                return;
            }
            let arr = current.as_array_mut().unwrap();
            while arr.len() <= idx {
                arr.push(Value::Object(Default::default()));
            }
            current = &mut arr[idx];
        } else {
            if !current.is_object() {
                return;
            }
            current = current
                .as_object_mut()
                .unwrap()
                .entry(segment.clone())
                .or_insert_with(|| Value::Object(Default::default()));
        }
    }
    if let Some(obj) = current.as_object_mut() {
        obj.insert(last_key.clone(), value);
    }
}
