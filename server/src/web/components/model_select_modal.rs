use dioxus::prelude::*;
use dioxus_i18n::t;
use mac_mgmt_common::model_source::{group_models, ModelEntry, ModelNode, ModelSource};
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
        .take(500)
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

    let entries_with_family = models;
    // Group by family, then by base model name (before the colon/size tag).
    let groups = group_models(
        entries_with_family.iter().map(|(e, _)| e.clone()).collect(),
        |entry| {
            let family = entries_with_family
                .iter()
                .find(|(e, _)| e.model_id == entry.model_id)
                .map(|(_, f)| f.clone())
                .unwrap_or_default();
            let family_group = if family.is_empty() {
                "Other".to_string()
            } else {
                titlecase(&family)
            };
            // Sub-group: base model name (strip :tag).
            let base = entry.model_id.split(':').next().unwrap_or(&entry.model_id);
            vec![family_group, base.to_string()]
        },
    );

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

    let groups = group_models(entries, |entry| {
        let provider = entry
            .full_model_id
            .split('/')
            .next()
            .unwrap_or("unknown")
            .to_string();
        let family = guess_model_family(&entry.model_id);
        let mut segs = vec![titlecase(&provider)];
        if !family.is_empty() {
            segs.push(family);
        }
        segs
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

    let groups = group_models(entries, |entry| {
        // OpenRouter IDs are like "anthropic/claude-sonnet-4-6"
        let parts: Vec<&str> = entry.model_id.splitn(2, '/').collect();
        if parts.len() == 2 {
            let provider = titlecase(parts[0]);
            let family = guess_model_family(parts[1]);
            let mut segs = vec![provider];
            if !family.is_empty() {
                segs.push(family);
            }
            segs
        } else {
            vec!["Other".to_string()]
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
    let groups = group_models(entries, |entry| {
        let family = guess_model_family(&entry.model_id);
        if family.is_empty() {
            vec![]
        } else {
            vec![family]
        }
    });

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

/// Heuristic: extract a model family name from a model ID.
/// E.g. "claude-sonnet-4-6" → "Claude 4", "gpt-5.4-mini" → "GPT 5",
/// "gemini-2.5-flash" → "Gemini 2.5", "llama-3.3-70b" → "Llama 3.3".
#[cfg(feature = "server")]
fn guess_model_family(model_id: &str) -> String {
    // Known prefix patterns: name-version
    let families: &[(&str, &str)] = &[
        ("claude-opus-4-7", "Claude 4.7"),
        ("claude-opus-4-6", "Claude 4.6"),
        ("claude-sonnet-4-6", "Claude 4.6"),
        ("claude-opus-4-5", "Claude 4.5"),
        ("claude-sonnet-4-5", "Claude 4.5"),
        ("claude-haiku-4-5", "Claude 4.5"),
        ("claude-opus-4", "Claude 4"),
        ("claude-sonnet-4", "Claude 4"),
        ("claude-3-5", "Claude 3.5"),
        ("claude-3", "Claude 3"),
        ("gpt-5.5", "GPT 5.5"),
        ("gpt-5.4", "GPT 5.4"),
        ("gpt-5", "GPT 5"),
        ("gpt-4o", "GPT 4o"),
        ("gpt-4.1", "GPT 4.1"),
        ("gpt-4", "GPT 4"),
        ("o4-", "O4"),
        ("o3-", "O3"),
        ("o1-", "O1"),
        ("gemini-3", "Gemini 3"),
        ("gemini-2.5", "Gemini 2.5"),
        ("gemini-2", "Gemini 2"),
        ("gemini-1.5", "Gemini 1.5"),
        ("llama-4", "Llama 4"),
        ("llama-3.3", "Llama 3.3"),
        ("llama-3.2", "Llama 3.2"),
        ("llama-3.1", "Llama 3.1"),
        ("llama-3", "Llama 3"),
        ("mistral-large", "Mistral Large"),
        ("mistral-small", "Mistral Small"),
        ("mistral-medium", "Mistral Medium"),
        ("deepseek-", "DeepSeek"),
        ("grok-3", "Grok 3"),
        ("grok-2", "Grok 2"),
        ("qwen", "Qwen"),
        ("phi", "Phi"),
    ];

    let lower = model_id.to_lowercase();
    for (prefix, family) in families {
        if lower.starts_with(prefix) {
            return family.to_string();
        }
    }
    String::new()
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
    let expanded: Signal<HashSet<String>> = use_signal(HashSet::new);

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
