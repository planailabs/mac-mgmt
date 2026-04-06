use dioxus::prelude::*;

use crate::models::CustomerConfig;

#[server]
async fn get_current_config(customer_id: String) -> Result<Option<CustomerConfig>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let config = sqlx::query_as::<_, CustomerConfig>(
        "SELECT * FROM customer_configs WHERE customer_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(config)
}

#[server]
async fn save_config(customer_id: String, config_json: String) -> Result<(), ServerFnError> {
    let json: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
    // Validate
    let _: mac_mgmt_common::CustomerConfig = serde_json::from_value(json.clone())
        .map_err(|e| ServerFnError::new(format!("invalid config: {e}")))?;

    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO customer_configs (customer_id, config_json) VALUES ($1, $2)")
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
    let schema = schemars::schema_for!(mac_mgmt_common::CustomerConfig);
    let value = serde_json::to_value(&schema)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(value)
}

#[component]
pub fn ConfigEditor(customer_id: String) -> Element {
    let cid = customer_id.clone();
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

    let cid_save = customer_id.clone();
    let on_save = move |evt: FormEvent| {
        evt.prevent_default();
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
            p { class: "text-red-600 text-sm mb-2", "{err}" }
        }

        div { class: "flex items-center gap-2 mb-3",
            label { class: "text-sm text-gray-600 flex items-center gap-1 cursor-pointer",
                input {
                    r#type: "checkbox",
                    checked: *raw_mode.read(),
                    onchange: move |evt| raw_mode.set(evt.checked()),
                }
                "Raw JSON"
            }
        }

        form { onsubmit: on_save,
            if *raw_mode.read() {
                textarea {
                    class: "w-full h-64 font-mono text-sm border border-gray-300 rounded p-2 mb-2",
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
                        p { class: "text-red-600 text-sm", "Failed to load schema: {e}" }
                        textarea {
                            class: "w-full h-64 font-mono text-sm border border-gray-300 rounded p-2 mb-2",
                            placeholder: "Paste JSON config here...",
                            value: "{editor_text}",
                            oninput: move |evt| editor_text.set(evt.value()),
                        }
                    },
                    None => rsx! { p { class: "text-sm", "Loading schema..." } },
                }}
            }
            button {
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                r#type: "submit",
                "Save Config"
            }
        }

        {match &*config.read() {
            Some(Ok(Some(cfg))) => {
                let saved_at = cfg.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                rsx! {
                    div { class: "mt-4",
                        p { class: "text-xs text-gray-500", "Last saved: {saved_at}" }
                    }
                }
            }
            Some(Ok(None)) => rsx! {
                p { class: "text-sm text-gray-500 mt-2", "No config saved yet." }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm mt-2", "Error: {e}" } },
            None => rsx! { p { class: "text-sm mt-2", "Loading..." } },
        }}
    }
}

/// Renders structured form sections from JSON Schema, keeping the JSON signal in sync.
#[component]
fn StructuredEditor(schema: serde_json::Value, json_text: Signal<String>) -> Element {
    let form_values: Signal<serde_json::Value> = use_signal(|| {
        serde_json::from_str(&json_text.read()).unwrap_or(serde_json::Value::Object(Default::default()))
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
                    details { class: "border border-gray-200 rounded",
                        key: "{section_name}",
                        open: form_values.read().get(&section_name).is_some(),
                        summary { class: "px-3 py-2 bg-gray-50 cursor-pointer font-medium text-sm hover:bg-gray-100",
                            "{section_name_clone}"
                        }
                        if !description.is_empty() {
                            p { class: "px-3 pt-1 text-xs text-gray-500", "{description}" }
                        }
                        div { class: "px-3 py-2 space-y-2",
                            {render_section_fields(
                                &resolved,
                                &defs,
                                section_name.clone(),
                                form_values,
                                sync_to_json,
                            )}
                        }
                    }
                }
            })}
        }
    }
}

/// Resolve a `$ref` pointer in the schema.
fn resolve_ref<'a>(
    schema: &'a serde_json::Value,
    defs: &'a serde_json::Value,
) -> std::borrow::Cow<'a, serde_json::Value> {
    if let Some(r) = schema.get("$ref").and_then(|r| r.as_str()) {
        // Refs look like "#/$defs/TypeName"
        if let Some(type_name) = r.strip_prefix("#/$defs/") {
            if let Some(resolved) = defs.get(type_name) {
                return std::borrow::Cow::Borrowed(resolved);
            }
        }
    }
    std::borrow::Cow::Borrowed(schema)
}

/// Render fields for one config section.
fn render_section_fields(
    section_schema: &serde_json::Value,
    defs: &serde_json::Value,
    section_name: String,
    mut form_values: Signal<serde_json::Value>,
    sync_to_json: impl Fn() + Clone + 'static,
) -> Element {
    let properties = section_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    rsx! {
        {properties.into_iter().map(|(field_name, field_schema)| {
            let resolved = resolve_ref(&field_schema, defs);
            let description = resolved
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();

            let field_type = resolved
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("string")
                .to_string();

            let current_value = form_values
                .read()
                .get(&section_name)
                .and_then(|s| s.get(&field_name))
                .cloned();

            let section_clone = section_name.clone();
            let field_clone = field_name.clone();
            let sync = sync_to_json.clone();

            rsx! {
                div { class: "flex flex-col gap-0.5",
                    key: "{section_name}-{field_name}",
                    label { class: "text-sm font-medium text-gray-700", "{field_clone}" }
                    if !description.is_empty() {
                        p { class: "text-xs text-gray-500", "{description}" }
                    }
                    {match field_type.as_str() {
                        "boolean" => {
                            let checked = current_value
                                .as_ref()
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let section_c = section_clone.clone();
                            let field_c = field_clone.clone();
                            let sync_c = sync.clone();
                            rsx! {
                                input {
                                    r#type: "checkbox",
                                    class: "h-4 w-4",
                                    checked: checked,
                                    onchange: move |evt| {
                                        set_field(&mut form_values, &section_c, &field_c,
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
                            let section_c = section_clone.clone();
                            let field_c = field_clone.clone();
                            let sync_c = sync.clone();
                            rsx! {
                                input {
                                    r#type: "number",
                                    class: "border border-gray-300 rounded px-2 py-1 text-sm w-full",
                                    value: val_str,
                                    oninput: move |evt| {
                                        if let Ok(n) = evt.value().parse::<i64>() {
                                            set_field(&mut form_values, &section_c, &field_c,
                                                serde_json::json!(n));
                                            sync_c();
                                        }
                                    },
                                }
                            }
                        }
                        "array" => {
                            let val_str = current_value
                                .as_ref()
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default();
                            let section_c = section_clone.clone();
                            let field_c = field_clone.clone();
                            let sync_c = sync.clone();
                            rsx! {
                                input {
                                    r#type: "text",
                                    class: "border border-gray-300 rounded px-2 py-1 text-sm w-full",
                                    placeholder: "comma-separated values",
                                    value: val_str,
                                    oninput: move |evt| {
                                        let arr: Vec<serde_json::Value> = evt.value()
                                            .split(',')
                                            .map(|s| serde_json::Value::String(s.trim().to_string()))
                                            .filter(|v| v.as_str() != Some(""))
                                            .collect();
                                        set_field(&mut form_values, &section_c, &field_c,
                                            serde_json::Value::Array(arr));
                                        sync_c();
                                    },
                                }
                            }
                        }
                        _ => {
                            // Check for enum values (string with enum constraint)
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
                            let section_c = section_clone.clone();
                            let field_c = field_clone.clone();
                            let sync_c = sync.clone();
                            if !enum_values.is_empty() {
                                rsx! {
                                    select {
                                        class: "border border-gray-300 rounded px-2 py-1 text-sm w-full",
                                        value: val_str,
                                        onchange: move |evt| {
                                            set_field(&mut form_values, &section_c, &field_c,
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
                                        class: "border border-gray-300 rounded px-2 py-1 text-sm w-full",
                                        value: val_str,
                                        oninput: move |evt| {
                                            let v = evt.value();
                                            if v.is_empty() {
                                                remove_field(&mut form_values, &section_c, &field_c);
                                            } else {
                                                set_field(&mut form_values, &section_c, &field_c,
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
        })}
    }
}

fn set_field(
    form_values: &mut Signal<serde_json::Value>,
    section: &str,
    field: &str,
    value: serde_json::Value,
) {
    let mut val = form_values.write();
    if let serde_json::Value::Object(root) = &mut *val {
        let section_obj = root
            .entry(section)
            .or_insert(serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(t) = section_obj {
            t.insert(field.to_string(), value);
        }
    }
}

fn remove_field(form_values: &mut Signal<serde_json::Value>, section: &str, field: &str) {
    let mut val = form_values.write();
    if let serde_json::Value::Object(root) = &mut *val {
        if let Some(serde_json::Value::Object(t)) = root.get_mut(section) {
            t.remove(field);
            if t.is_empty() {
                root.remove(section);
            }
        }
    }
}
