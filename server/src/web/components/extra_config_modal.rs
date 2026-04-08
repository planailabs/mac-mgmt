use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

const OPENCLAW_BASELINE: &str = include_str!("../../../ext/openclaw-config-baseline.json");

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawEntry {
    pub path: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default, rename = "type", deserialize_with = "deserialize_type")]
    pub ty: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub has_children: bool,
}

/// Accept `"type": "string"`, `"type": ["integer","string"]`, or missing.
/// For unions we pick the first concrete type — good enough for picking a
/// renderer.
fn deserialize_type<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let v = serde_json::Value::deserialize(d)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Array(arr) => Ok(arr
            .into_iter()
            .find_map(|v| v.as_str().map(String::from))
            .unwrap_or_default()),
        serde_json::Value::Null => Ok(String::new()),
        other => Err(D::Error::custom(format!("unexpected type field: {other}"))),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Baseline {
    #[serde(default)]
    core_entries: Vec<OpenClawEntry>,
    #[serde(default)]
    channel_entries: Vec<OpenClawEntry>,
    #[serde(default)]
    plugin_entries: Vec<OpenClawEntry>,
}

#[server]
pub async fn get_openclaw_schema() -> Result<Vec<OpenClawEntry>, ServerFnError> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Vec<OpenClawEntry>> = OnceLock::new();
    if let Some(cached) = CACHE.get() {
        return Ok(cached.clone());
    }
    let parsed: Baseline = serde_json::from_str(OPENCLAW_BASELINE)
        .map_err(|e| ServerFnError::new(format!("parse openclaw baseline: {e}")))?;
    let mut entries = parsed.core_entries;
    entries.extend(parsed.channel_entries);
    entries.extend(parsed.plugin_entries);
    let _ = CACHE.set(entries.clone());
    Ok(entries)
}

// ── Tree ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
struct Node {
    name: String,
    full_path: String,
    entry: Option<OpenClawEntry>,
    children: Vec<Node>,
    array_item: Option<Box<Node>>,
}

fn build_tree(entries: &[OpenClawEntry]) -> Node {
    let mut root = Node::default();
    for e in entries {
        if e.deprecated {
            continue;
        }
        let segments: Vec<&str> = e.path.split('.').collect();
        insert(&mut root, &segments, e.clone());
    }
    sort_tree(&mut root);
    root
}

fn insert(node: &mut Node, segments: &[&str], entry: OpenClawEntry) {
    if segments.is_empty() {
        node.entry = Some(entry);
        return;
    }
    let head = segments[0];
    let rest = &segments[1..];
    if head == "*" {
        if node.array_item.is_none() {
            let full_path = if node.full_path.is_empty() {
                "*".to_string()
            } else {
                format!("{}.*", node.full_path)
            };
            node.array_item = Some(Box::new(Node {
                name: "*".into(),
                full_path,
                ..Default::default()
            }));
        }
        insert(node.array_item.as_mut().unwrap(), rest, entry);
        return;
    }
    if let Some(idx) = node.children.iter().position(|c| c.name == head) {
        insert(&mut node.children[idx], rest, entry);
        return;
    }
    let full_path = if node.full_path.is_empty() {
        head.to_string()
    } else {
        format!("{}.{}", node.full_path, head)
    };
    node.children.push(Node {
        name: head.to_string(),
        full_path,
        ..Default::default()
    });
    let last_idx = node.children.len() - 1;
    insert(&mut node.children[last_idx], rest, entry);
}

fn sort_tree(node: &mut Node) {
    node.children.sort_by(|a, b| a.name.cmp(&b.name));
    for c in &mut node.children {
        sort_tree(c);
    }
    if let Some(item) = node.array_item.as_mut() {
        sort_tree(item);
    }
}

// ── Component ───────────────────────────────────────────────────────────

/// Inline trigger button shown inside the structured config form. The
/// modal itself is rendered at a higher level (StructuredEditor) so it
/// stays mounted regardless of whether the openclaw section is collapsed.
#[component]
pub fn ExtraConfigField(
    form_values: Signal<serde_json::Value>,
    mut open: Signal<bool>,
) -> Element {
    let path = vec!["openclaw".to_string(), "extra_config".to_string()];
    let current = get_at(&form_values.read(), &path).unwrap_or(serde_json::Value::Null);
    let key_count = current
        .as_object()
        .map(|o| count_leaves(&serde_json::Value::Object(o.clone())))
        .unwrap_or(0);

    rsx! {
        div { class: "flex flex-col gap-0.5",
            label { class: "text-sm font-medium text-gray-700", "extra_config" }
            p { class: "text-xs text-gray-500",
                "Arbitrary openclaw.json keys merged after typed fields."
            }
            div {
                button {
                    r#type: "button",
                    class: "bg-indigo-600 text-white px-3 py-1 rounded text-sm hover:bg-indigo-700",
                    onclick: move |evt| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        open.set(true);
                    },
                    "Edit extra_config…"
                }
                span { class: "ml-2 text-xs text-gray-500", "{key_count} value(s) set" }
            }
        }
    }
}

/// Top-level mount point for the extra_config modal. Render once at the
/// StructuredEditor level so opening/closing it is independent of any
/// collapsible section in the form below.
#[component]
pub fn ExtraConfigModalHost(
    mut open: Signal<bool>,
    mut form_values: Signal<serde_json::Value>,
    mut json_text: Signal<String>,
) -> Element {
    if !*open.read() {
        return rsx! {};
    }
    let path = vec!["openclaw".to_string(), "extra_config".to_string()];
    let initial = get_at(&form_values.read(), &path).unwrap_or(serde_json::Value::Null);
    rsx! {
        ExtraConfigModal {
            open,
            initial,
            on_save: move |new_value: serde_json::Value| {
                if new_value.as_object().map(|o| o.is_empty()).unwrap_or(false)
                    || new_value.is_null()
                {
                    remove_at(&mut form_values, &path);
                } else {
                    set_at(&mut form_values, &path, new_value);
                }
                let snapshot = form_values.read().clone();
                json_text.set(serde_json::to_string_pretty(&snapshot).unwrap_or_default());
                open.set(false);
            },
        }
    }
}

fn count_leaves(v: &serde_json::Value) -> usize {
    match v {
        serde_json::Value::Object(map) => map.values().map(count_leaves).sum(),
        serde_json::Value::Array(arr) => arr.iter().map(count_leaves).sum(),
        serde_json::Value::Null => 0,
        _ => 1,
    }
}

#[component]
fn ExtraConfigModal(
    open: Signal<bool>,
    initial: serde_json::Value,
    on_save: EventHandler<serde_json::Value>,
) -> Element {
    let schema = use_server_future(|| async { get_openclaw_schema().await })?;
    let working: Signal<serde_json::Value> = use_signal(|| {
        if initial.is_object() {
            initial.clone()
        } else {
            serde_json::Value::Object(Default::default())
        }
    });
    let mut filter = use_signal(String::new);

    let tree = match &*schema.read() {
        Some(Ok(entries)) => Some(build_tree(entries)),
        _ => None,
    };

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50",
            onclick: move |_| open.set(false),
            div {
                class: "bg-white rounded-lg shadow-xl max-w-3xl w-full max-h-[85vh] flex flex-col",
                onclick: move |e| e.stop_propagation(),

                div { class: "px-4 py-3 border-b flex items-center justify-between",
                    h2 { class: "font-semibold text-base", "Edit openclaw extra_config" }
                    button {
                        r#type: "button",
                        class: "text-gray-500 hover:text-gray-800 text-xl leading-none",
                        onclick: move |evt| {
                            evt.prevent_default();
                            evt.stop_propagation();
                            open.set(false);
                        },
                        "×"
                    }
                }

                div { class: "px-4 py-2 border-b",
                    input {
                        r#type: "text",
                        class: "w-full border border-gray-300 rounded px-2 py-1 text-sm",
                        placeholder: "Filter by path…",
                        value: "{filter}",
                        oninput: move |e| filter.set(e.value()),
                    }
                }

                div { class: "px-4 py-3 overflow-y-auto flex-1 space-y-2",
                    {match (&*schema.read(), tree) {
                        (Some(Ok(_)), Some(root)) => {
                            let filter_str = filter.read().to_lowercase();
                            rsx! {
                                {
                                    root.children
                                        .into_iter()
                                        .filter(|c| node_matches(c, &filter_str))
                                        .map(|child| {
                                            let p = vec![child.name.clone()];
                                            render_node(child, p, working, &filter_str)
                                        })
                                }
                            }
                        }
                        (Some(Err(e)), _) => rsx! {
                            p { class: "text-red-600 text-sm", "Failed to load schema: {e}" }
                        },
                        _ => rsx! { p { class: "text-sm", "Loading schema…" } },
                    }}
                }

                div { class: "px-4 py-3 border-t flex justify-end gap-2",
                    button {
                        r#type: "button",
                        class: "px-3 py-1 rounded text-sm border border-gray-300 hover:bg-gray-100",
                        onclick: move |evt| {
                            evt.prevent_default();
                            evt.stop_propagation();
                            open.set(false);
                        },
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "px-3 py-1 rounded text-sm bg-blue-600 text-white hover:bg-blue-700",
                        onclick: move |evt| {
                            evt.prevent_default();
                            evt.stop_propagation();
                            let cleaned = prune(working.read().clone());
                            on_save.call(cleaned);
                        },
                        "Save"
                    }
                }
            }
        }
    }
}

/// Drop empty objects/arrays/strings/null leaves so we don't bloat extra_config.
fn prune(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map {
                let pruned = prune(val);
                if !is_empty(&pruned) {
                    out.insert(k, pruned);
                }
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            let pruned: Vec<_> = arr.into_iter().map(prune).filter(|v| !is_empty(v)).collect();
            serde_json::Value::Array(pruned)
        }
        other => other,
    }
}

fn is_empty(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.is_empty(),
        serde_json::Value::Object(m) => m.is_empty(),
        serde_json::Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

// ── Path helpers (index-aware) ──────────────────────────────────────────
//
// Path segments that parse as `usize` are treated as array indices; all
// other segments as object keys. This lets us address values nested under
// arrays of objects (e.g. `messages.tts.providers.0.apiKey`).

pub(super) fn get_at(root: &serde_json::Value, path: &[String]) -> Option<serde_json::Value> {
    let mut cur = root;
    for seg in path {
        if let Ok(idx) = seg.parse::<usize>() {
            cur = cur.as_array()?.get(idx)?;
        } else {
            cur = cur.get(seg)?;
        }
    }
    Some(cur.clone())
}

pub(super) fn set_at(
    working: &mut Signal<serde_json::Value>,
    path: &[String],
    value: serde_json::Value,
) {
    if path.is_empty() {
        working.set(value);
        return;
    }
    let mut val = working.write();
    let mut cur = &mut *val;
    for seg in &path[..path.len() - 1] {
        if let Ok(idx) = seg.parse::<usize>() {
            if !cur.is_array() {
                *cur = serde_json::Value::Array(vec![]);
            }
            let arr = cur.as_array_mut().unwrap();
            while arr.len() <= idx {
                arr.push(serde_json::Value::Null);
            }
            cur = &mut arr[idx];
        } else {
            if !cur.is_object() {
                *cur = serde_json::Value::Object(serde_json::Map::new());
            }
            cur = cur
                .as_object_mut()
                .unwrap()
                .entry(seg.clone())
                .or_insert(serde_json::Value::Null);
        }
    }
    let last = path.last().unwrap();
    if let Ok(idx) = last.parse::<usize>() {
        if !cur.is_array() {
            *cur = serde_json::Value::Array(vec![]);
        }
        let arr = cur.as_array_mut().unwrap();
        while arr.len() <= idx {
            arr.push(serde_json::Value::Null);
        }
        arr[idx] = value;
    } else {
        if !cur.is_object() {
            *cur = serde_json::Value::Object(serde_json::Map::new());
        }
        cur.as_object_mut().unwrap().insert(last.clone(), value);
    }
}

pub(super) fn remove_at(working: &mut Signal<serde_json::Value>, path: &[String]) {
    if path.is_empty() {
        working.set(serde_json::Value::Null);
        return;
    }
    let mut val = working.write();
    let mut cur = &mut *val;
    for seg in &path[..path.len() - 1] {
        let next = if let Ok(idx) = seg.parse::<usize>() {
            cur.as_array_mut().and_then(|a| a.get_mut(idx))
        } else {
            cur.get_mut(seg)
        };
        match next {
            Some(n) => cur = n,
            None => return,
        }
    }
    let last = path.last().unwrap();
    if let Ok(idx) = last.parse::<usize>() {
        if let Some(arr) = cur.as_array_mut() {
            if idx < arr.len() {
                arr.remove(idx);
            }
        }
    } else if let Some(obj) = cur.as_object_mut() {
        obj.remove(last);
    }
}

// ── Node rendering ──────────────────────────────────────────────────────

fn node_matches(node: &Node, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let hay_path = node.full_path.to_lowercase();
    let hay_name = node.name.to_lowercase();
    let hay_label = node
        .entry
        .as_ref()
        .and_then(|e| e.label.as_deref())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    let hay_help = node
        .entry
        .as_ref()
        .and_then(|e| e.help.as_deref())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    // Tokenize filter on whitespace; every token must hit somewhere in the
    // node OR in a descendant subtree.
    filter.split_whitespace().all(|tok| {
        hay_path.contains(tok)
            || hay_name.contains(tok)
            || hay_label.contains(tok)
            || hay_help.contains(tok)
            || node.children.iter().any(|c| token_in_subtree(c, tok))
            || node
                .array_item
                .as_ref()
                .map(|a| token_in_subtree(a, tok))
                .unwrap_or(false)
    })
}

fn token_in_subtree(node: &Node, tok: &str) -> bool {
    if node.full_path.to_lowercase().contains(tok)
        || node.name.to_lowercase().contains(tok)
        || node
            .entry
            .as_ref()
            .and_then(|e| e.label.as_deref())
            .map(|s| s.to_lowercase().contains(tok))
            .unwrap_or(false)
        || node
            .entry
            .as_ref()
            .and_then(|e| e.help.as_deref())
            .map(|s| s.to_lowercase().contains(tok))
            .unwrap_or(false)
    {
        return true;
    }
    node.children.iter().any(|c| token_in_subtree(c, tok))
        || node
            .array_item
            .as_ref()
            .map(|a| token_in_subtree(a, tok))
            .unwrap_or(false)
}

fn render_node(
    node: Node,
    json_path: Vec<String>,
    mut working: Signal<serde_json::Value>,
    filter: &str,
) -> Element {
    let entry_ty = node.entry.as_ref().map(|e| e.ty.as_str()).unwrap_or("");
    let title = node
        .entry
        .as_ref()
        .and_then(|e| e.label.clone())
        .unwrap_or_else(|| node.name.clone());
    let help = node
        .entry
        .as_ref()
        .and_then(|e| e.help.clone())
        .unwrap_or_default();
    let path = json_path;
    let key = node.full_path.clone();

    // Object map: object/empty parent whose only child is a wildcard `*`
    // (i.e. a string-keyed map<K, V> in the openclaw schema).
    if node.children.is_empty()
        && node.array_item.is_some()
        && (entry_ty == "object" || entry_ty.is_empty() || entry_ty == "any")
    {
        let template = node.array_item.clone().unwrap();
        let has_value = get_at(&working.read(), &path)
            .map(|v| !is_empty(&v))
            .unwrap_or(false);
        let map_path = path.clone();
        let filter_owned = filter.to_string();
        return rsx! {
            details { class: "border border-gray-200 rounded",
                key: "{key}",
                open: !filter.is_empty() || has_value,
                summary { class: "px-2 py-1 bg-gray-50 cursor-pointer text-sm font-semibold hover:bg-gray-100",
                    "{title} "
                    span { class: "text-gray-400 font-normal text-xs", "({node.name})" }
                }
                if !help.is_empty() {
                    p { class: "px-2 pt-1 text-xs text-gray-500", "{help}" }
                }
                div { class: "px-2 py-2 space-y-2",
                    {render_object_map(template, map_path, working, &filter_owned)}
                }
            }
        };
    }

    // Object / branch with children: collapsible group
    if !node.children.is_empty() && (entry_ty == "object" || entry_ty.is_empty() || entry_ty == "any") {
        let filter_owned = filter.to_string();
        let has_value = get_at(&working.read(), &path)
            .map(|v| !is_empty(&v))
            .unwrap_or(false);
        let parent_path = path.clone();
        return rsx! {
            details { class: "border border-gray-200 rounded",
                key: "{key}",
                open: !filter.is_empty() || has_value,
                summary { class: "px-2 py-1 bg-gray-50 cursor-pointer text-sm font-semibold hover:bg-gray-100",
                    "{title} "
                    span { class: "text-gray-400 font-normal text-xs", "({node.name})" }
                }
                if !help.is_empty() {
                    p { class: "px-2 pt-1 text-xs text-gray-500", "{help}" }
                }
                div { class: "px-2 py-2 space-y-2",
                    {
                        let filter_for_filter = filter_owned.clone();
                        node.children
                            .into_iter()
                            .filter(move |c| node_matches(c, &filter_for_filter))
                            .map(move |c| {
                                let mut p = parent_path.clone();
                                p.push(c.name.clone());
                                render_node(c, p, working, &filter_owned)
                            })
                    }
                }
            }
        };
    }

    // Leaf
    let current = get_at(&working.read(), &path);
    let sensitive = node.entry.as_ref().map(|e| e.sensitive).unwrap_or(false);

    let field = match entry_ty {
        "boolean" => {
            let checked = current.as_ref().and_then(|v| v.as_bool()).unwrap_or(false);
            let path_c = path.clone();
            rsx! {
                input {
                    r#type: "checkbox",
                    class: "h-4 w-4",
                    checked,
                    onchange: move |e| {
                        set_at(&mut working, &path_c, serde_json::Value::Bool(e.checked()));
                    },
                }
            }
        }
        "integer" => {
            let val_str = current
                .as_ref()
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            let path_c = path.clone();
            let path_d = path.clone();
            rsx! {
                input {
                    r#type: "number",
                    class: "border border-gray-300 rounded px-2 py-1 text-sm w-full",
                    value: val_str,
                    oninput: move |e| {
                        let v = e.value();
                        if v.is_empty() {
                            remove_at(&mut working, &path_d);
                        } else if let Ok(n) = v.parse::<i64>() {
                            set_at(&mut working, &path_c, serde_json::json!(n));
                        }
                    },
                }
            }
        }
        "number" => {
            let val_str = current
                .as_ref()
                .and_then(|v| v.as_f64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            let path_c = path.clone();
            let path_d = path.clone();
            rsx! {
                input {
                    r#type: "number",
                    step: "any",
                    class: "border border-gray-300 rounded px-2 py-1 text-sm w-full",
                    value: val_str,
                    oninput: move |e| {
                        let v = e.value();
                        if v.is_empty() {
                            remove_at(&mut working, &path_d);
                        } else if let Ok(n) = v.parse::<f64>() {
                            set_at(
                                &mut working,
                                &path_c,
                                serde_json::Number::from_f64(n)
                                    .map(serde_json::Value::Number)
                                    .unwrap_or(serde_json::Value::Null),
                            );
                        }
                    },
                }
            }
        }
        "array" => {
            let item = node.array_item.clone();
            let item_ty = item
                .as_ref()
                .and_then(|n| n.entry.as_ref())
                .map(|e| e.ty.clone())
                .unwrap_or_default();
            let item_has_children =
                item.as_ref().map(|n| !n.children.is_empty()).unwrap_or(false);
            if item_has_children || item_ty == "object" {
                render_object_array(item.unwrap(), path.clone(), working, filter)
            } else if matches!(
                item_ty.as_str(),
                "string" | "number" | "integer" | "boolean" | ""
            ) {
                render_primitive_array(path.clone(), current.clone(), working, &item_ty)
            } else {
                render_json_textarea(path.clone(), current.clone(), working)
            }
        }
        "string" => {
            let val_str = current
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let path_c = path.clone();
            let path_d = path.clone();
            rsx! {
                input {
                    r#type: if sensitive { "password" } else { "text" },
                    class: "border border-gray-300 rounded px-2 py-1 text-sm w-full",
                    value: val_str,
                    oninput: move |e| {
                        let v = e.value();
                        if v.is_empty() {
                            remove_at(&mut working, &path_d);
                        } else {
                            set_at(&mut working, &path_c, serde_json::Value::String(v));
                        }
                    },
                }
            }
        }
        _ => render_json_textarea(path.clone(), current.clone(), working),
    };

    let is_set = current.is_some();
    let reset_path = path.clone();

    rsx! {
        div { class: "flex flex-col gap-0.5",
            key: "{key}",
            div { class: "flex items-center justify-between gap-2",
                label { class: "text-sm font-medium text-gray-700",
                    "{title} "
                    span { class: "text-gray-400 font-normal text-xs", "({node.name})" }
                }
                if is_set {
                    button {
                        r#type: "button",
                        class: "text-xs text-gray-500 hover:text-red-600 underline",
                        onclick: move |evt| {
                            evt.prevent_default();
                            evt.stop_propagation();
                            remove_at(&mut working, &reset_path);
                        },
                        "reset to default"
                    }
                }
            }
            if !help.is_empty() {
                p { class: "text-xs text-gray-500", "{help}" }
            }
            {field}
        }
    }
}

/// Render a string-keyed map (openclaw `object` parent whose only child
/// is a wildcard `*`). Each existing key becomes a removable collapsible
/// group rendering the `*` template's children at `[…path, key]`. A key
/// input + "add" button inserts new entries.
fn render_object_map(
    template: Box<Node>,
    path: Vec<String>,
    mut working: Signal<serde_json::Value>,
    filter: &str,
) -> Element {
    let keys: Vec<String> = get_at(&working.read(), &path)
        .and_then(|v| v.as_object().map(|o| o.keys().cloned().collect()))
        .unwrap_or_default();
    let mut new_key = use_signal(String::new);
    let path_for_add = path.clone();
    let template_for_add = template.clone();
    let filter_outer = filter.to_string();

    rsx! {
        div { class: "space-y-2 border-l-2 border-amber-300 pl-2",
            for k in keys {
                {
                    let mut entry_path = path.clone();
                    entry_path.push(k.clone());
                    let entry_path_for_remove = entry_path.clone();
                    let template_node = (*template).clone();
                    let leaf_filter = filter_outer.clone();
                    let branch_filter_a = filter_outer.clone();
                    let branch_filter_b = filter_outer.clone();
                    let parent_path = entry_path.clone();
                    let dom_key = format!("{}.{}", path.join("."), k);
                    let label = k.clone();
                    rsx! {
                        details {
                            key: "{dom_key}",
                            class: "border border-gray-200 rounded",
                            open: true,
                            summary { class: "px-2 py-1 bg-gray-50 cursor-pointer text-sm font-semibold flex items-center justify-between",
                                span { "{label}" }
                                button {
                                    r#type: "button",
                                    class: "text-red-500 hover:text-red-700 text-xs",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        remove_at(&mut working, &entry_path_for_remove);
                                    },
                                    "remove"
                                }
                            }
                            div { class: "px-2 py-2 space-y-2",
                                if template_node.children.is_empty() {
                                    {render_node(template_node, parent_path, working, &leaf_filter)}
                                } else {
                                    {
                                        template_node.children
                                            .into_iter()
                                            .filter(move |c| node_matches(c, &branch_filter_a))
                                            .map(move |c| {
                                                let mut p = parent_path.clone();
                                                p.push(c.name.clone());
                                                render_node(c, p, working, &branch_filter_b)
                                            })
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "flex gap-1",
                input {
                    r#type: "text",
                    class: "flex-1 border border-gray-300 rounded px-2 py-1 text-sm",
                    placeholder: "key",
                    value: "{new_key}",
                    oninput: move |e| new_key.set(e.value()),
                }
                button {
                    r#type: "button",
                    class: "bg-blue-600 text-white px-2 py-1 rounded text-xs hover:bg-blue-700",
                    onclick: move |evt| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        let k = new_key.read().trim().to_string();
                        if k.is_empty() {
                            return;
                        }
                        let mut entry_path = path_for_add.clone();
                        entry_path.push(k);
                        let init = if template_for_add.children.is_empty() {
                            match template_for_add
                                .entry
                                .as_ref()
                                .map(|e| e.ty.as_str())
                                .unwrap_or("")
                            {
                                "string" => serde_json::Value::String(String::new()),
                                "boolean" => serde_json::Value::Bool(false),
                                "integer" | "number" => serde_json::json!(0),
                                "array" => serde_json::Value::Array(vec![]),
                                _ => serde_json::Value::Null,
                            }
                        } else {
                            serde_json::Value::Object(Default::default())
                        };
                        set_at(&mut working, &entry_path, init);
                        new_key.set(String::new());
                    },
                    "+ add"
                }
            }
        }
    }
}

/// Render an array whose `*` template is an object (or otherwise has
/// children). Each existing array entry becomes a numbered, removable
/// collapsible group; an "add item" button appends an empty object.
fn render_object_array(
    array_item: Box<Node>,
    path: Vec<String>,
    mut working: Signal<serde_json::Value>,
    filter: &str,
) -> Element {
    let arr_len = get_at(&working.read(), &path)
        .and_then(|v| v.as_array().map(|a| a.len()))
        .unwrap_or(0);
    let filter_owned = filter.to_string();
    let path_for_add = path.clone();

    rsx! {
        div { class: "space-y-2 border-l-2 border-emerald-300 pl-2",
            for idx in 0..arr_len {
                {
                    let mut item_path = path.clone();
                    item_path.push(idx.to_string());
                    let key = format!("{}#{idx}", path.join("."));
                    let item_path_for_remove = item_path.clone();
                    let item_node = (*array_item).clone();
                    let item_filter_outer = filter_owned.clone();
                    let item_filter_inner = filter_owned.clone();
                    let parent_path = item_path.clone();
                    rsx! {
                        details {
                            key: "{key}",
                            class: "border border-gray-200 rounded",
                            open: true,
                            summary { class: "px-2 py-1 bg-gray-50 cursor-pointer text-sm font-semibold flex items-center justify-between",
                                span { "[{idx}]" }
                                button {
                                    r#type: "button",
                                    class: "text-red-500 hover:text-red-700 text-xs",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        remove_at(&mut working, &item_path_for_remove);
                                    },
                                    "remove"
                                }
                            }
                            div { class: "px-2 py-2 space-y-2",
                                {
                                    let filter_for_filter = item_filter_outer.clone();
                                    item_node.children
                                        .into_iter()
                                        .filter(move |c| node_matches(c, &filter_for_filter))
                                        .map(move |c| {
                                            let mut p = parent_path.clone();
                                            p.push(c.name.clone());
                                            render_node(c, p, working, &item_filter_inner)
                                        })
                                }
                            }
                        }
                    }
                }
            }
            button {
                r#type: "button",
                class: "bg-blue-600 text-white px-2 py-1 rounded text-xs hover:bg-blue-700",
                onclick: move |evt| {
                    evt.prevent_default();
                    evt.stop_propagation();
                    let mut arr = get_at(&working.read(), &path_for_add)
                        .and_then(|v| v.as_array().cloned())
                        .unwrap_or_default();
                    arr.push(serde_json::Value::Object(Default::default()));
                    set_at(&mut working, &path_for_add, serde_json::Value::Array(arr));
                },
                "+ add item"
            }
        }
    }
}

/// Parse a free-text input into a JSON value matching `item_ty`. For
/// integer/number we try to parse and fall back to a string if the input
/// isn't numeric — this matches openclaw union types like
/// `["number", "string"]` for fields such as `allowFrom`.
fn parse_primitive(input: &str, item_ty: &str) -> serde_json::Value {
    match item_ty {
        "boolean" => match input.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => serde_json::Value::Bool(true),
            "false" | "no" | "0" => serde_json::Value::Bool(false),
            _ => serde_json::Value::String(input.to_string()),
        },
        "integer" => input
            .trim()
            .parse::<i64>()
            .map(|n| serde_json::json!(n))
            .unwrap_or_else(|_| serde_json::Value::String(input.to_string())),
        "number" => input
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(input.to_string())),
        _ => serde_json::Value::String(input.to_string()),
    }
}

fn render_primitive_array(
    path: Vec<String>,
    current: Option<serde_json::Value>,
    mut working: Signal<serde_json::Value>,
    item_ty: &str,
) -> Element {
    let items: Vec<String> = current
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut new_val = use_signal(String::new);
    let item_ty_owned = item_ty.to_string();

    let add_path = path.clone();
    let add_ty = item_ty_owned.clone();
    let add = move |_| {
        let val = new_val.read().trim().to_string();
        if val.is_empty() {
            return;
        }
        let mut arr = get_at(&working.read(), &add_path)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        arr.push(parse_primitive(&val, &add_ty));
        set_at(&mut working, &add_path, serde_json::Value::Array(arr));
        new_val.set(String::new());
    };

    rsx! {
        div { class: "space-y-1",
            for (idx, item) in items.iter().enumerate() {
                div {
                    key: "{idx}",
                    class: "flex items-center gap-1",
                    span { class: "flex-1 text-sm font-mono bg-gray-50 border border-gray-200 rounded px-2 py-0.5 truncate",
                        "{item}"
                    }
                    button {
                        r#type: "button",
                        class: "text-red-500 hover:text-red-700 text-xs px-1",
                        onclick: {
                            let path = path.clone();
                            move |_| {
                                let mut arr = get_at(&working.read(), &path)
                                    .and_then(|v| v.as_array().cloned())
                                    .unwrap_or_default();
                                if idx < arr.len() {
                                    arr.remove(idx);
                                }
                                set_at(&mut working, &path, serde_json::Value::Array(arr));
                            }
                        },
                        "x"
                    }
                }
            }
            div { class: "flex gap-1",
                input {
                    r#type: "text",
                    class: "flex-1 border border-gray-300 rounded px-2 py-0.5 text-sm",
                    placeholder: "Add item…",
                    value: "{new_val}",
                    oninput: move |e| new_val.set(e.value()),
                }
                button {
                    r#type: "button",
                    class: "bg-blue-600 text-white px-2 py-0.5 rounded text-xs hover:bg-blue-700",
                    onclick: add,
                    "+"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(path: &str, ty: &str) -> OpenClawEntry {
        OpenClawEntry {
            path: path.to_string(),
            kind: "core".into(),
            ty: ty.to_string(),
            required: false,
            deprecated: false,
            sensitive: false,
            tags: vec![],
            label: None,
            help: None,
            has_children: false,
        }
    }

    #[test]
    fn build_tree_groups_dotted_paths() {
        let entries = vec![
            entry("acp", "object"),
            entry("acp.enabled", "boolean"),
            entry("acp.backend", "string"),
            entry("acp.allowedAgents", "array"),
            entry("acp.allowedAgents.*", "string"),
            entry("telemetry.port", "integer"),
        ];
        let root = build_tree(&entries);
        assert_eq!(root.children.len(), 2);
        let acp = root.children.iter().find(|c| c.name == "acp").unwrap();
        assert_eq!(acp.entry.as_ref().unwrap().ty, "object");
        // children are sorted alphabetically
        let names: Vec<_> = acp.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["allowedAgents", "backend", "enabled"]);
        let allowed = acp.children.iter().find(|c| c.name == "allowedAgents").unwrap();
        assert_eq!(allowed.entry.as_ref().unwrap().ty, "array");
        let item = allowed.array_item.as_ref().expect("* child");
        assert_eq!(item.entry.as_ref().unwrap().ty, "string");
        assert_eq!(item.full_path, "acp.allowedAgents.*");
    }

    #[test]
    fn build_tree_skips_deprecated() {
        let mut e = entry("legacy.flag", "boolean");
        e.deprecated = true;
        let root = build_tree(&[e, entry("kept", "string")]);
        let names: Vec<_> = root.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["kept"]);
    }

    #[test]
    fn prune_drops_empty_branches() {
        let input = json!({
            "a": "",
            "b": null,
            "c": {},
            "d": [],
            "e": {
                "inner": "",
                "kept": "value",
            },
            "f": [null, "x", ""],
            "g": false,
            "h": 0,
        });
        let out = prune(input);
        assert_eq!(
            out,
            json!({
                "e": { "kept": "value" },
                "f": ["x"],
                "g": false,
                "h": 0,
            })
        );
    }

    #[test]
    fn build_tree_keeps_object_array_template() {
        let entries = vec![
            entry("providers", "array"),
            entry("providers.*", "object"),
            entry("providers.*.apiKey", "string"),
            entry("providers.*.region", "string"),
        ];
        let root = build_tree(&entries);
        let providers = root.children.iter().find(|c| c.name == "providers").unwrap();
        let item = providers.array_item.as_ref().expect("array_item");
        assert_eq!(item.entry.as_ref().unwrap().ty, "object");
        let names: Vec<_> = item.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["apiKey", "region"]);
    }

    #[test]
    fn parse_primitive_handles_union_fallback() {
        // integer with numeric input → number
        assert_eq!(parse_primitive("42", "integer"), json!(42));
        // integer with non-numeric input → falls back to string (matches
        // ["number","string"] union semantics for fields like allowFrom)
        assert_eq!(parse_primitive("@user", "integer"), json!("@user"));
        assert_eq!(parse_primitive("3.14", "number"), json!(3.14));
        assert_eq!(parse_primitive("true", "boolean"), json!(true));
        assert_eq!(parse_primitive("hello", "string"), json!("hello"));
    }

    #[test]
    fn parses_full_embedded_baseline() {
        let parsed: Baseline = serde_json::from_str(OPENCLAW_BASELINE)
            .expect("embedded baseline must parse");
        let total = parsed.core_entries.len()
            + parsed.channel_entries.len()
            + parsed.plugin_entries.len();
        assert!(total > 100, "expected many entries, got {total}");
        // sanity: a known core entry exists
        assert!(parsed.core_entries.iter().any(|e| e.path == "acp.enabled"));

        // tree builds without panicking and groups by top-level key
        let mut all = parsed.core_entries;
        all.extend(parsed.channel_entries);
        all.extend(parsed.plugin_entries);
        let root = build_tree(&all);
        assert!(!root.children.is_empty());
        assert!(root.children.iter().any(|c| c.name == "acp"));
    }

    #[test]
    fn get_at_handles_array_indices() {
        let v = json!({"providers": [{"apiKey": "k1"}, {"apiKey": "k2"}]});
        assert_eq!(
            get_at(&v, &["providers".into(), "0".into(), "apiKey".into()]),
            Some(json!("k1"))
        );
        assert_eq!(
            get_at(&v, &["providers".into(), "1".into(), "apiKey".into()]),
            Some(json!("k2"))
        );
        assert_eq!(get_at(&v, &["providers".into(), "5".into()]), None);
    }

    #[test]
    fn set_at_creates_arrays_and_objects() {
        // Smoke-test the structural mutation paths through a hand-rolled
        // mock by going through the actual signal API would require a
        // dioxus runtime; instead exercise the same logic on a Value.
        let mut root = serde_json::Value::Null;
        // Inline the same logic as set_at, since set_at takes a Signal.
        fn write(root: &mut serde_json::Value, path: &[&str], value: serde_json::Value) {
            let mut cur = root;
            for seg in &path[..path.len() - 1] {
                if let Ok(idx) = seg.parse::<usize>() {
                    if !cur.is_array() {
                        *cur = serde_json::Value::Array(vec![]);
                    }
                    let arr = cur.as_array_mut().unwrap();
                    while arr.len() <= idx {
                        arr.push(serde_json::Value::Null);
                    }
                    cur = &mut arr[idx];
                } else {
                    if !cur.is_object() {
                        *cur = serde_json::Value::Object(serde_json::Map::new());
                    }
                    cur = cur
                        .as_object_mut()
                        .unwrap()
                        .entry((*seg).to_string())
                        .or_insert(serde_json::Value::Null);
                }
            }
            let last = path.last().unwrap();
            if let Ok(idx) = last.parse::<usize>() {
                if !cur.is_array() {
                    *cur = serde_json::Value::Array(vec![]);
                }
                let arr = cur.as_array_mut().unwrap();
                while arr.len() <= idx {
                    arr.push(serde_json::Value::Null);
                }
                arr[idx] = value;
            } else {
                if !cur.is_object() {
                    *cur = serde_json::Value::Object(serde_json::Map::new());
                }
                cur.as_object_mut()
                    .unwrap()
                    .insert((*last).to_string(), value);
            }
        }
        write(&mut root, &["providers", "0", "apiKey"], json!("k1"));
        write(&mut root, &["providers", "1", "apiKey"], json!("k2"));
        assert_eq!(
            root,
            json!({"providers": [{"apiKey": "k1"}, {"apiKey": "k2"}]})
        );
    }

    #[test]
    fn prune_preserves_nested_objects_with_values() {
        let input = json!({"acp": {"enabled": true, "backend": ""}});
        assert_eq!(prune(input), json!({"acp": {"enabled": true}}));
    }
}

fn render_json_textarea(
    path: Vec<String>,
    current: Option<serde_json::Value>,
    mut working: Signal<serde_json::Value>,
) -> Element {
    let text = current
        .as_ref()
        .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
        .unwrap_or_default();
    let path_c = path.clone();
    let path_d = path.clone();
    rsx! {
        textarea {
            class: "w-full h-24 font-mono text-xs border border-gray-300 rounded p-1",
            placeholder: "JSON value",
            value: "{text}",
            oninput: move |e| {
                let v = e.value();
                if v.trim().is_empty() {
                    remove_at(&mut working, &path_d);
                } else if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&v) {
                    set_at(&mut working, &path_c, parsed);
                }
            },
        }
    }
}
