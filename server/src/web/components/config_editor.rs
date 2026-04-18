use dioxus::prelude::*;

use crate::models::ClusterConfig;
#[cfg(feature = "server")]
use crate::web::user::current_user;
use super::extra_config_modal::{ExtraConfigField, ExtraConfigModalHost};

#[server]
async fn get_current_config(cluster_id: String) -> Result<Option<ClusterConfig>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let config = sqlx::query_as::<_, ClusterConfig>(
        "SELECT * FROM cluster_configs WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(config.map(|mut c| {
        mac_mgmt_common::config_migrate::migrate(&mut c.config_json);
        c
    }))
}

#[server]
async fn save_config(cluster_id: String, config_json: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let json: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
    // Validate
    let _: mac_mgmt_common::ClusterConfig = serde_json::from_value(json.clone())
        .map_err(|e| ServerFnError::new(format!("invalid config: {e}")))?;

    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.writable_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    sqlx::query("INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)")
        .bind(uuid)
        .bind(&json)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

#[server]
async fn get_config_schema() -> Result<serde_json::Value, ServerFnError> {
    let _user = current_user().await?;
    let schema = schemars::schema_for!(mac_mgmt_common::ClusterConfig);
    let value = serde_json::to_value(&schema)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(value)
}

#[component]
pub fn ConfigEditor(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut config = use_server_future(move || {
        let cid = cid.clone();
        async move { get_current_config(cid).await }
    })?;

    let schema = use_server_future(|| async { get_config_schema().await })?;

    let mut editor_text = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut initialized = use_signal(|| false);
    let mut raw_mode = use_signal(|| false);

    if !*initialized.read() {
        if let Some(Ok(Some(cfg))) = &*config.read() {
            editor_text.set(serde_json::to_string_pretty(&cfg.config_json).unwrap_or_default());
            initialized.set(true);
        }
    }

    let cid_save = cluster_id.clone();
    let do_save = move || {
        let cid = cid_save.clone();
        let text = editor_text.read().clone();
        spawn(async move {
            match save_config(cid, text).await {
                Ok(()) => {
                    error.set(None);
                    config.restart();
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 dark:text-red-400 text-sm mb-2", "{err}" }
        }

        div { class: "flex items-center gap-2 mb-3",
            label { class: "text-sm text-gray-600 dark:text-gray-300 flex items-center gap-1 cursor-pointer",
                input {
                    r#type: "checkbox",
                    checked: *raw_mode.read(),
                    onchange: move |evt| raw_mode.set(evt.checked()),
                }
                "Raw JSON"
            }
        }

        div {
            if *raw_mode.read() {
                textarea {
                    class: "w-full h-64 font-mono text-sm border border-gray-300 dark:border-gray-600 rounded p-2 mb-2 dark:bg-gray-700 dark:text-white",
                    placeholder: "Paste JSON config here...",
                    value: "{editor_text}",
                    oninput: move |evt| editor_text.set(evt.value()),
                }
            } else {
                {match &*schema.read() {
                    Some(Ok(schema_val)) => {
                        rsx! {
                            StructuredEditor {
                                schema: schema_val.clone(),
                                json_text: editor_text,
                            }
                        }
                    }
                    Some(Err(e)) => rsx! {
                        p { class: "text-red-600 dark:text-red-400 text-sm", "Failed to load schema: {e}" }
                        textarea {
                            class: "w-full h-64 font-mono text-sm border border-gray-300 dark:border-gray-600 rounded p-2 mb-2 dark:bg-gray-700 dark:text-white",
                            placeholder: "Paste JSON config here...",
                            value: "{editor_text}",
                            oninput: move |evt| editor_text.set(evt.value()),
                        }
                    },
                    None => rsx! { p { class: "text-sm", "Loading schema..." } },
                }}
            }
            if !read_only {
                button {
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    r#type: "button",
                    onclick: move |evt| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        do_save();
                    },
                    "Save Config"
                }
            }
        }

        {match &*config.read() {
            Some(Ok(Some(cfg))) => {
                let saved_at = cfg.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                rsx! {
                    div { class: "mt-4",
                        p { class: "text-xs text-gray-500 dark:text-gray-400", "Last saved: {saved_at}" }
                    }
                }
            }
            Some(Ok(None)) => rsx! {
                p { class: "text-sm text-gray-500 dark:text-gray-400 mt-2", "No config saved yet." }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm mt-2", "Error: {e}" } },
            None => rsx! { p { class: "text-sm mt-2", "Loading..." } },
        }}
    }
}

/// Renders structured form sections from JSON Schema, keeping the JSON signal in sync.
#[component]
fn StructuredEditor(schema: serde_json::Value, json_text: Signal<String>) -> Element {
    let mut form_values: Signal<serde_json::Value> = use_signal(|| serde_json::Value::Object(Default::default()));
    let extra_config_open = use_signal(|| false);

    // Keep form_values in sync when json_text changes (e.g. after config loads)
    use_effect(move || {
        let text = json_text.read().clone();
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
            form_values.set(parsed);
        }
    });

    let sync_to_json = move || {
        let json = form_values.read().clone();
        let mut text = json_text;
        text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
    };

    // Get schema properties (top-level sections)
    let properties = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    // Resolve $defs for nested types
    let defs = schema
        .get("$defs")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    rsx! {
        ExtraConfigModalHost {
            open: extra_config_open,
            form_values,
            json_text,
        }
        div { class: "space-y-3 mb-3",
            {properties.into_iter().map(|(section_name, section_schema)| {
                let resolved = resolve_ref(&section_schema, &defs);
                let description = resolved
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();

                let section_name_clone = section_name.clone();
                rsx! {
                    details { class: "border border-gray-300 dark:border-gray-600 rounded shadow-sm",
                        key: "{section_name}",
                        open: form_values.read().get(&section_name).is_some(),
                        summary { class: "px-3 py-2 bg-gray-100 dark:bg-gray-700 cursor-pointer font-semibold text-sm hover:bg-gray-200 dark:hover:bg-gray-600",
                            "{section_name_clone}"
                        }
                        if !description.is_empty() {
                            p { class: "px-3 pt-1 text-xs text-gray-500 dark:text-gray-400", "{description}" }
                        }
                        div { class: "px-3 py-2 space-y-2",
                            {render_section_fields(
                                &resolved,
                                &defs,
                                vec![section_name.clone()],
                                form_values,
                                json_text,
                                extra_config_open,
                                sync_to_json,
                            )}
                        }
                    }
                }
            })}
        }
    }
}

/// Resolve a `$ref` pointer in the schema, including `anyOf` wrappers
/// from `Option<T>` which schemars generates as `anyOf: [{$ref: ...}, {type: "null"}]`.
fn resolve_ref(
    schema: &serde_json::Value,
    defs: &serde_json::Value,
) -> serde_json::Value {
    // Direct $ref
    if let Some(r) = schema.get("$ref").and_then(|r| r.as_str()) {
        if let Some(type_name) = r.strip_prefix("#/$defs/") {
            if let Some(resolved) = defs.get(type_name) {
                return resolved.clone();
            }
        }
    }
    // anyOf wrapper (Option<T> → anyOf: [{$ref: "..."}, {type: "null"}])
    if let Some(any_of) = schema.get("anyOf").and_then(|a| a.as_array()) {
        for variant in any_of {
            if variant.get("type").and_then(|t| t.as_str()) == Some("null") {
                continue;
            }
            let resolved = resolve_ref(variant, defs);
            if resolved.get("properties").is_some() || resolved.get("type").is_some() {
                return resolved;
            }
        }
    }
    schema.clone()
}

/// Render fields for one config section, including nested subsections.
fn render_section_fields(
    section_schema: &serde_json::Value,
    defs: &serde_json::Value,
    path: Vec<String>,
    mut form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    sync_to_json: impl Fn() + Clone + 'static,
) -> Element {
    let properties = section_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    rsx! {
        {properties.into_iter().map(|(field_name, field_schema)| {
            // Special-case: extra_config under openclaw is rendered via the
            // openclaw-baseline-driven modal, not the schemars schema.
            if field_name == "extra_config" && path.last().map(|s| s.as_str()) == Some("openclaw") {
                return rsx! {
                    ExtraConfigField {
                        key: "{field_name}",
                        form_values,
                        open: extra_config_open,
                    }
                };
            }
            let resolved = resolve_ref(&field_schema, defs);
            // Description may be on the field schema itself (for $ref fields)
            // or on the resolved type definition
            let description = field_schema
                .get("description")
                .or_else(|| resolved.get("description"))
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();

            let field_type = resolved
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("string")
                .to_string();

            // Check if this field is a nested object (subsection)
            let is_object = field_type == "object"
                || resolved.get("properties").is_some();

            let mut field_path = path.clone();
            field_path.push(field_name.clone());

            let current_value = get_at_path(&form_values.read(), &field_path);

            let key = field_path.join(".");
            let sync = sync_to_json.clone();

            if is_object {
                // Render as a nested subsection
                let defs_clone = defs.clone();
                rsx! {
                    div { class: "border-l-2 border-blue-300 pl-3 mt-2 mb-1",
                        key: "{key}",
                        label { class: "text-sm font-semibold text-blue-700 dark:text-blue-400", "{field_name}" }
                        if !description.is_empty() {
                            p { class: "text-xs text-gray-500 dark:text-gray-400", "{description}" }
                        }
                        {render_section_fields(
                            &resolved,
                            &defs_clone,
                            field_path,
                            form_values,
                            json_text,
                            extra_config_open,
                            sync,
                        )}
                    }
                }
            } else {
                let fp = field_path.clone();
                let fp2 = field_path.clone();
                rsx! {
                    div { class: "flex flex-col gap-0.5",
                        key: "{key}",
                        label { class: "text-sm font-medium text-gray-700 dark:text-gray-200", "{field_name}" }
                        if !description.is_empty() {
                            p { class: "text-xs text-gray-500 dark:text-gray-400", "{description}" }
                        }
                        {match field_type.as_str() {
                            "boolean" => {
                                let checked = current_value
                                    .as_ref()
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                let fp = fp.clone();
                                let sync_c = sync.clone();
                                rsx! {
                                    input {
                                        r#type: "checkbox",
                                        class: "h-4 w-4",
                                        checked: checked,
                                        onchange: move |evt| {
                                            set_at_path(&mut form_values, &fp,
                                                serde_json::Value::Bool(evt.checked()));
                                            sync_c();
                                        },
                                    }
                                }
                            }
                            "integer" => {
                                let val_str = current_value
                                    .as_ref()
                                    .and_then(|v| v.as_i64())
                                    .map(|n| n.to_string())
                                    .unwrap_or_default();
                                let fp = fp.clone();
                                let sync_c = sync.clone();
                                rsx! {
                                    input {
                                        r#type: "number",
                                        class: "border border-gray-300 dark:border-gray-600 rounded px-2 dark:bg-gray-700 dark:text-white py-1 text-sm w-full",
                                        value: val_str,
                                        oninput: move |evt| {
                                            if let Ok(n) = evt.value().parse::<i64>() {
                                                set_at_path(&mut form_values, &fp,
                                                    serde_json::json!(n));
                                                sync_c();
                                            }
                                        },
                                    }
                                }
                            }
                            "array" => {
                                // Resolve items schema to distinguish string arrays from object arrays.
                                let items_schema = resolved.get("items")
                                    .map(|s| resolve_ref(s, defs))
                                    .unwrap_or_default();
                                let is_object_array = items_schema.get("properties").is_some();

                                if is_object_array {
                                    // ── Array of objects (e.g. cloud providers list) ──
                                    let entries: Vec<serde_json::Value> = current_value
                                        .as_ref()
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    let fp_remove = fp.clone();
                                    let fp_add = fp.clone();
                                    let sync_remove = sync.clone();
                                    let sync_add = sync.clone();
                                    let items_schema_add = items_schema.clone();
                                    let defs_add = defs.clone();
                                    rsx! {
                                        div { class: "space-y-2",
                                            for (idx, _entry) in entries.iter().enumerate() {
                                                {
                                                    let defs_c = defs.clone();
                                                    let items_c = items_schema.clone();
                                                    let fp_r = fp_remove.clone();
                                                    let sync_r = sync_remove.clone();
                                                    let mut entry_path = fp_remove.clone();
                                                    entry_path.push(format!("{idx}"));
                                                    rsx! {
                                                        div { class: "border border-gray-300 dark:border-gray-600 rounded p-2 relative",
                                                            key: "{idx}",
                                                            div { class: "flex justify-between items-center mb-1",
                                                                span { class: "text-xs font-semibold text-gray-500 dark:text-gray-400", "#{idx}" }
                                                                button {
                                                                    class: "text-red-500 hover:text-red-700 text-xs px-1",
                                                                    r#type: "button",
                                                                    onclick: move |_| {
                                                                        let mut arr = get_at_path(&form_values.read(), &fp_r)
                                                                            .and_then(|v| v.as_array().cloned())
                                                                            .unwrap_or_default();
                                                                        if idx < arr.len() {
                                                                            arr.remove(idx);
                                                                        }
                                                                        set_at_path(&mut form_values, &fp_r,
                                                                            serde_json::Value::Array(arr));
                                                                        sync_r();
                                                                    },
                                                                    "Remove"
                                                                }
                                                            }
                                                            {render_object_array_entry(
                                                                &items_c,
                                                                &defs_c,
                                                                entry_path,
                                                                form_values,
                                                                json_text,
                                                                extra_config_open,
                                                                sync_remove.clone(),
                                                            )}
                                                        }
                                                    }
                                                }
                                            }
                                            button {
                                                r#type: "button",
                                                class: "bg-green-600 text-white px-3 py-1 rounded text-xs hover:bg-green-700",
                                                onclick: move |_| {
                                                    let mut arr = get_at_path(&form_values.read(), &fp_add)
                                                        .and_then(|v| v.as_array().cloned())
                                                        .unwrap_or_default();
                                                    // Add an empty object; defaults come from schema.
                                                    let new_obj = build_default_object(&items_schema_add, &defs_add);
                                                    arr.push(new_obj);
                                                    set_at_path(&mut form_values, &fp_add,
                                                        serde_json::Value::Array(arr));
                                                    sync_add();
                                                },
                                                "+ Add entry"
                                            }
                                        }
                                    }
                                } else {
                                    // ── Array of primitives (strings, numbers) ──
                                    let items: Vec<String> = current_value
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
                                    let fp_add = fp.clone();
                                    let fp_remove = fp.clone();
                                    let sync_add = sync.clone();
                                    let sync_remove = sync.clone();
                                    rsx! {
                                        div { class: "space-y-1",
                                            for (idx, item) in items.iter().enumerate() {
                                                div {
                                                    key: "{idx}",
                                                    class: "flex items-center gap-1",
                                                    span { class: "flex-1 text-sm font-mono bg-gray-50 dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded px-2 py-0.5 truncate",
                                                        "{item}"
                                                    }
                                                    button {
                                                        class: "text-red-500 hover:text-red-700 text-xs px-1",
                                                        r#type: "button",
                                                        onclick: {
                                                            let fp = fp_remove.clone();
                                                            let sync_c = sync_remove.clone();
                                                            move |_| {
                                                                let mut arr = get_at_path(&form_values.read(), &fp)
                                                                    .and_then(|v| v.as_array().cloned())
                                                                    .unwrap_or_default();
                                                                if idx < arr.len() {
                                                                    arr.remove(idx);
                                                                }
                                                                set_at_path(&mut form_values, &fp,
                                                                    serde_json::Value::Array(arr));
                                                                sync_c();
                                                            }
                                                        },
                                                        "x"
                                                    }
                                                }
                                            }
                                            // Add new item
                                            {
                                                let fp = fp_add.clone();
                                                let sync_c = sync_add.clone();
                                                let mut new_val = use_signal(String::new);
                                                rsx! {
                                                    div { class: "flex gap-1",
                                                        input {
                                                            r#type: "text",
                                                            class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-2 py-0.5 text-sm dark:bg-gray-700 dark:text-white",
                                                            placeholder: "Add item...",
                                                            value: "{new_val}",
                                                            oninput: move |e| new_val.set(e.value()),
                                                            onkeypress: {
                                                                let fp = fp.clone();
                                                                let sync_c = sync_c.clone();
                                                                move |e: KeyboardEvent| {
                                                                    if e.key() == Key::Enter {
                                                                        let val = new_val.read().clone();
                                                                        if !val.trim().is_empty() {
                                                                            let mut arr = get_at_path(&form_values.read(), &fp)
                                                                                .and_then(|v| v.as_array().cloned())
                                                                                .unwrap_or_default();
                                                                            arr.push(serde_json::Value::String(val.trim().to_string()));
                                                                            set_at_path(&mut form_values, &fp,
                                                                                serde_json::Value::Array(arr));
                                                                            sync_c();
                                                                            new_val.set(String::new());
                                                                        }
                                                                    }
                                                                }
                                                            },
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "bg-blue-600 text-white px-2 py-0.5 rounded text-xs hover:bg-blue-700",
                                                            onclick: {
                                                                let fp = fp.clone();
                                                                let sync_c = sync_c.clone();
                                                                move |_| {
                                                                    let val = new_val.read().clone();
                                                                    if !val.trim().is_empty() {
                                                                        let mut arr = get_at_path(&form_values.read(), &fp)
                                                                            .and_then(|v| v.as_array().cloned())
                                                                            .unwrap_or_default();
                                                                        arr.push(serde_json::Value::String(val.trim().to_string()));
                                                                        set_at_path(&mut form_values, &fp,
                                                                            serde_json::Value::Array(arr));
                                                                        sync_c();
                                                                        new_val.set(String::new());
                                                                    }
                                                                }
                                                            },
                                                            "+"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {
                                let enum_values: Vec<String> = resolved
                                    .get("enum")
                                    .and_then(|e| e.as_array())
                                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                    .unwrap_or_default();
                                let val_str = current_value
                                    .as_ref()
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let sync_c = sync.clone();
                                if !enum_values.is_empty() {
                                    let fp = fp.clone();
                                    rsx! {
                                        select {
                                            class: "border border-gray-300 dark:border-gray-600 rounded px-2 dark:bg-gray-700 dark:text-white py-1 text-sm w-full",
                                            value: val_str,
                                            onchange: move |evt| {
                                                set_at_path(&mut form_values, &fp,
                                                    serde_json::Value::String(evt.value()));
                                                sync_c();
                                            },
                                            option { value: "", "-- select --" }
                                            {enum_values.iter().map(|v| {
                                                let v = v.clone();
                                                rsx! { option { value: "{v}", "{v}" } }
                                            })}
                                        }
                                    }
                                } else {
                                    rsx! {
                                        input {
                                            r#type: "text",
                                            class: "border border-gray-300 dark:border-gray-600 rounded px-2 dark:bg-gray-700 dark:text-white py-1 text-sm w-full",
                                            value: val_str,
                                            oninput: move |evt| {
                                                let v = evt.value();
                                                if v.is_empty() {
                                                    remove_at_path(&mut form_values, &fp2);
                                                } else {
                                                    set_at_path(&mut form_values, &fp2,
                                                        serde_json::Value::String(v));
                                                }
                                                sync_c();
                                            },
                                        }
                                    }
                                }
                            }
                        }}
                    }
                }
            }
        })}
    }
}

/// Get a value at a nested JSON path. Numeric segments index into arrays.
pub(super) fn get_at_path(root: &serde_json::Value, path: &[String]) -> Option<serde_json::Value> {
    let mut current = root;
    for key in path {
        if let Ok(idx) = key.parse::<usize>() {
            current = current.as_array()?.get(idx)?;
        } else {
            current = current.get(key)?;
        }
    }
    Some(current.clone())
}

/// Set a value at a nested JSON path, creating intermediate objects as needed.
/// Numeric path segments index into arrays.
pub(super) fn set_at_path(
    form_values: &mut Signal<serde_json::Value>,
    path: &[String],
    value: serde_json::Value,
) {
    let mut val = form_values.write();
    let mut current = &mut *val;
    for key in &path[..path.len() - 1] {
        if let Ok(idx) = key.parse::<usize>() {
            if let serde_json::Value::Array(arr) = current {
                while arr.len() <= idx {
                    arr.push(serde_json::Value::Object(serde_json::Map::new()));
                }
                current = &mut arr[idx];
                continue;
            }
        }
        current = current
            .as_object_mut()
            .unwrap()
            .entry(key)
            .or_insert(serde_json::Value::Object(serde_json::Map::new()));
    }
    if let Some(last) = path.last() {
        if let Ok(idx) = last.parse::<usize>() {
            if let serde_json::Value::Array(arr) = current {
                while arr.len() <= idx {
                    arr.push(serde_json::Value::Null);
                }
                arr[idx] = value;
            }
        } else if let serde_json::Value::Object(obj) = current {
            obj.insert(last.clone(), value);
        }
    }
}

/// Remove a value at a nested JSON path.
pub(super) fn remove_at_path(form_values: &mut Signal<serde_json::Value>, path: &[String]) {
    let mut val = form_values.write();
    let mut current = &mut *val;
    for key in &path[..path.len() - 1] {
        match current.get_mut(key) {
            Some(next) => current = next,
            None => return,
        }
    }
    if let Some(last) = path.last() {
        if let serde_json::Value::Object(obj) = current {
            obj.remove(last);
        }
    }
}

/// Render a single entry inside an object array (e.g. one cloud provider entry).
/// Re-uses the same field-rendering logic as `render_section_fields` but rooted
/// at the array-element path (e.g. `["cloud", "0"]`).
fn render_object_array_entry(
    items_schema: &serde_json::Value,
    defs: &serde_json::Value,
    entry_path: Vec<String>,
    form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    sync_to_json: impl Fn() + Clone + 'static,
) -> Element {
    render_section_fields(
        items_schema,
        defs,
        entry_path,
        form_values,
        json_text,
        extra_config_open,
        sync_to_json,
    )
}

/// Build a default JSON object from a schema (one level deep, for "add entry").
fn build_default_object(
    schema: &serde_json::Value,
    defs: &serde_json::Value,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (key, prop_schema) in props {
            let resolved = resolve_ref(prop_schema, defs);
            if let Some(default) = resolved.get("default") {
                obj.insert(key.clone(), default.clone());
            } else {
                let field_type = resolved
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("string");
                match field_type {
                    "boolean" => { obj.insert(key.clone(), serde_json::Value::Bool(true)); }
                    "string" => {}
                    _ => {}
                }
            }
        }
    }
    serde_json::Value::Object(obj)
}
