use dioxus::prelude::*;
use dioxus_i18n::t;
use mac_mgmt_common::model_source::{ModelEntry, ModelNode, ModelSource};
use std::collections::HashSet;

// ── Server functions ─────────────────────────────────────────────────

/// Load a model source from the pre-generated static catalog.
/// The catalog is embedded at compile time from `server/ext/model-catalog.json`.
#[server]
pub async fn get_model_catalog(source_id: String) -> Result<ModelSource, ServerFnError> {
    use std::sync::OnceLock;

    static CATALOG: OnceLock<Vec<ModelSource>> = OnceLock::new();

    let sources = CATALOG.get_or_init(|| {
        let json = include_str!("../../../ext/model-catalog.json");
        let catalog: crate::model_catalog_fetch::ModelCatalog =
            serde_json::from_str(json).expect("failed to parse embedded model-catalog.json");
        catalog.sources
    });

    sources
        .iter()
        .find(|s| s.id == source_id)
        .cloned()
        .ok_or_else(|| {
            ServerFnError::new(format!(
                "source '{}' not found in catalog (available: {})",
                source_id,
                sources.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(", ")
            ))
        })
}

/// Fetch models from the OpenClaw gateway (live, not from static catalog).
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
                ..Default::default()
            }
        })
        .collect();

    let groups = crate::model_catalog_fetch::auto_group(entries, |entry| {
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
    let gw_host = req.gateway_host.clone();
    let gw_port = req.gateway_port;

    let catalog = use_server_future(move || {
        let sk = source_kind.clone();
        let prov = provider.clone();
        let gh = gw_host.clone();
        let gp = gw_port;
        async move {
            match sk.as_str() {
                // Sources served from the static catalog
                "ollama" | "openrouter" | "lms" => {
                    let catalog_id = if sk == "lms" { "ollama".to_string() } else { sk.clone() };
                    get_model_catalog(catalog_id).await
                }
                // OpenClaw is fetched live from the local gateway
                "openclaw" => {
                    let host = gh.unwrap_or_else(|| "127.0.0.1".into());
                    let port = gp.unwrap_or(18789);
                    fetch_openclaw_models(host, port, None).await
                }
                // Cloud providers: look up by provider slug in the static catalog
                "cloud" => {
                    let catalog_id = prov.unwrap_or_else(|| sk.clone());
                    get_model_catalog(catalog_id).await.or_else(|_| {
                        Ok(ModelSource {
                            id: "cloud".into(),
                            display_name: "Cloud".into(),
                            groups: vec![],
                        })
                    })
                }
                // Try the static catalog for any other source_kind
                _ => get_model_catalog(sk.clone()).await.or_else(|_| {
                    Ok(ModelSource {
                        id: sk.clone(),
                        display_name: sk,
                        groups: vec![],
                    })
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
