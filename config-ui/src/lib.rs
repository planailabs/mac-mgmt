//! Shared, schema-driven config editor used by BOTH mac-mgmt-server and the
//! sovereign-AI USB overview app — one editor, not a clone.
//!
//! The component is decoupled from any data layer: callers pass the JSON Schema
//! (`schemars::schema_for!(...)` output), the current config JSON, and an
//! `on_save` handler that persists the edited JSON however they like (the
//! server via its `#[server]` fn / postgres; the overview via `PUT /config` on
//! the loopback control API). This is why it can run in a desktop app, which
//! has no Dioxus fullstack `#[server]` functions.

use dioxus::prelude::*;
use plan_ai_design::{Button, ButtonSize, ButtonVariant, Card};
use serde_json::Value;

/// A shared config editor.
///
/// - `schema`  — JSON Schema describing the config (sections + field types).
/// - `initial` — the current config as JSON.
/// - `read_only` — disable editing/saving.
/// - `saving` — show the save button as busy.
/// - `error` — an error message from the last save attempt.
/// - `on_save` — called with the edited config as a JSON string.
#[component]
pub fn ConfigEditor(
    schema: Value,
    initial: Value,
    #[props(default)] read_only: bool,
    #[props(default)] saving: bool,
    #[props(default)] error: Option<String>,
    on_save: EventHandler<String>,
) -> Element {
    let mut form = use_signal(|| normalize(&initial));
    let saved = use_signal(|| normalize(&initial));
    let mut raw_mode = use_signal(|| false);
    let mut raw_text =
        use_signal(|| serde_json::to_string_pretty(&initial).unwrap_or_else(|_| "{}".into()));
    let mut raw_err = use_signal(|| None::<String>);

    let dirty = *form.read() != *saved.read();
    let sections = top_level_sections(&schema);

    let do_save = move |_| {
        let json = if *raw_mode.read() {
            match serde_json::from_str::<Value>(&raw_text.read()) {
                Ok(_) => raw_text.read().clone(),
                Err(e) => {
                    raw_err.set(Some(format!("invalid JSON: {e}")));
                    return;
                }
            }
        } else {
            serde_json::to_string_pretty(&*form.read()).unwrap_or_default()
        };
        raw_err.set(None);
        on_save.call(json);
    };

    let mut discard = move |_| {
        form.set(saved.read().clone());
        raw_text.set(serde_json::to_string_pretty(&*saved.read()).unwrap_or_default());
        raw_err.set(None);
    };

    rsx! {
        div { class: "config-editor",
            div { class: "config-editor-bar",
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| {
                        let entering_raw = !*raw_mode.read();
                        if entering_raw {
                            raw_text.set(serde_json::to_string_pretty(&*form.read()).unwrap_or_default());
                        } else if let Ok(v) = serde_json::from_str::<Value>(&raw_text.read()) {
                            form.set(v);
                        }
                        raw_mode.toggle();
                    },
                    {if *raw_mode.read() { "Form view" } else { "Raw JSON" }}
                }
                if dirty && !read_only {
                    span { class: "config-editor-dirty", "● unsaved" }
                }
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Secondary,
                    disabled: read_only || !dirty,
                    onclick: move |e| discard(e),
                    "Discard"
                }
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Primary,
                    disabled: read_only || saving,
                    onclick: do_save,
                    {if saving { "Saving…" } else { "Save" }}
                }
            }

            if let Some(err) = error {
                p { class: "config-editor-error", "{err}" }
            }
            if let Some(err) = raw_err.read().clone() {
                p { class: "config-editor-error", "{err}" }
            }

            if *raw_mode.read() {
                textarea {
                    class: "config-editor-raw",
                    readonly: read_only,
                    value: "{raw_text}",
                    oninput: move |e| raw_text.set(e.value()),
                }
            } else {
                for section in sections {
                    SectionCard { form, schema: schema.clone(), section, read_only }
                }
            }
        }
    }
}

/// One config section (a top-level object property) rendered as a card of
/// scalar field inputs; nested objects fall back to a JSON textarea.
#[component]
fn SectionCard(
    form: Signal<Value>,
    schema: Value,
    section: SectionMeta,
    read_only: bool,
) -> Element {
    let fields = section_fields(&schema, &section.key);
    rsx! {
        Card {
            div { class: "config-section",
                h3 { "{section.title}" }
                if let Some(desc) = section.description.clone() {
                    p { class: "config-section-desc", "{desc}" }
                }
                for field in fields {
                    FieldRow { form, section: section.key.clone(), field, read_only }
                }
            }
        }
    }
}

/// A single field input, dispatched by its JSON-schema type.
#[component]
fn FieldRow(form: Signal<Value>, section: String, field: FieldMeta, read_only: bool) -> Element {
    let cur = get_field(&form.read(), &section, &field.key);
    let (sec1, key1) = (section.clone(), field.key.clone());
    let (sec2, key2) = (section.clone(), field.key.clone());

    rsx! {
        label { class: "config-field",
            span { class: "config-field-label",
                "{field.title}"
                if let Some(d) = field.description.clone() {
                    span { class: "config-field-help", title: "{d}", " ⓘ" }
                }
            }
            match field.kind {
                FieldKind::Bool => rsx! {
                    input {
                        r#type: "checkbox",
                        disabled: read_only,
                        checked: cur.as_ref().and_then(|v| v.as_bool()).unwrap_or(false),
                        onchange: move |e| {
                            let mut f = form;
                            set_field(&mut f, &sec1, &key1, Value::Bool(e.checked()));
                        },
                    }
                },
                FieldKind::Number => rsx! {
                    input {
                        r#type: "number",
                        disabled: read_only,
                        value: cur.as_ref().map(scalar_text).unwrap_or_default(),
                        oninput: move |e| {
                            let mut f = form;
                            let v = e.value();
                            let parsed = v.parse::<i64>().map(|n| Value::from(n))
                                .or_else(|_| v.parse::<f64>().map(Value::from))
                                .unwrap_or(Value::Null);
                            set_field(&mut f, &sec2, &key2, parsed);
                        },
                    }
                },
                FieldKind::Text => rsx! {
                    input {
                        r#type: "text",
                        disabled: read_only,
                        value: cur.as_ref().map(scalar_text).unwrap_or_default(),
                        oninput: move |e| {
                            let mut f = form;
                            set_field(&mut f, &sec1, &key1, Value::String(e.value()));
                        },
                    }
                },
                FieldKind::Json => rsx! {
                    textarea {
                        class: "config-field-json",
                        disabled: read_only,
                        value: cur.as_ref().map(|v| serde_json::to_string_pretty(v).unwrap_or_default()).unwrap_or_default(),
                        oninput: move |e| {
                            let mut f = form;
                            if let Ok(v) = serde_json::from_str::<Value>(&e.value()) {
                                set_field(&mut f, &sec2, &key2, v);
                            }
                        },
                    }
                },
            }
        }
    }
}

// ── Schema introspection ────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
struct SectionMeta {
    key: String,
    title: String,
    description: Option<String>,
}

#[derive(Clone, PartialEq)]
struct FieldMeta {
    key: String,
    title: String,
    description: Option<String>,
    kind: FieldKind,
}

#[derive(Clone, Copy, PartialEq)]
enum FieldKind {
    Bool,
    Number,
    Text,
    Json,
}

/// Resolve a `$ref` against the schema's `$defs`/`definitions`, one level.
fn resolve(schema: &Value, node: &Value) -> Value {
    if let Some(r) = node.get("$ref").and_then(|v| v.as_str()) {
        let name = r.rsplit('/').next().unwrap_or("");
        for defs_key in ["$defs", "definitions"] {
            if let Some(def) = schema.get(defs_key).and_then(|d| d.get(name)) {
                return def.clone();
            }
        }
    }
    node.clone()
}

/// Top-level config sections, in schema order.
fn top_level_sections(schema: &Value) -> Vec<SectionMeta> {
    let props = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return Vec::new(),
    };
    props
        .iter()
        .map(|(k, v)| {
            let resolved = resolve(schema, v);
            SectionMeta {
                key: k.clone(),
                title: title_for(k, &resolved),
                description: resolved
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(str::to_string),
            }
        })
        .collect()
}

/// The scalar/json fields of a section (one level deep).
fn section_fields(schema: &Value, section: &str) -> Vec<FieldMeta> {
    let Some(sec) = schema
        .get("properties")
        .and_then(|p| p.get(section))
        .map(|v| resolve(schema, v))
    else {
        return Vec::new();
    };
    let Some(props) = sec.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    props
        .iter()
        .map(|(k, v)| {
            let resolved = resolve(schema, v);
            FieldMeta {
                key: k.clone(),
                title: title_for(k, &resolved),
                description: resolved
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(str::to_string),
                kind: kind_for(&resolved),
            }
        })
        .collect()
}

fn kind_for(schema: &Value) -> FieldKind {
    // schemars emits "type" as a string or array (for nullable). Look at both.
    let ty = schema.get("type");
    let has = |name: &str| match ty {
        Some(Value::String(s)) => s == name,
        Some(Value::Array(a)) => a.iter().any(|x| x.as_str() == Some(name)),
        _ => false,
    };
    if has("boolean") {
        FieldKind::Bool
    } else if has("integer") || has("number") {
        FieldKind::Number
    } else if has("string") {
        FieldKind::Text
    } else {
        FieldKind::Json
    }
}

fn title_for(key: &str, schema: &Value) -> String {
    if let Some(t) = schema.get("title").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    // Humanize the key: snake_case -> Title Case.
    key.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Form value helpers ──────────────────────────────────────────────────

/// Ensure the working value is an object.
fn normalize(v: &Value) -> Value {
    if v.is_object() {
        v.clone()
    } else {
        Value::Object(serde_json::Map::new())
    }
}

fn get_field(form: &Value, section: &str, field: &str) -> Option<Value> {
    form.get(section)?.get(field).cloned()
}

fn set_field(form: &mut Signal<Value>, section: &str, field: &str, val: Value) {
    let mut guard = form.write();
    if !guard.is_object() {
        *guard = Value::Object(serde_json::Map::new());
    }
    let obj = guard.as_object_mut().unwrap();
    let sec = obj
        .entry(section.to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if !sec.is_object() {
        *sec = Value::Object(serde_json::Map::new());
    }
    sec.as_object_mut()
        .unwrap()
        .insert(field.to_string(), val);
}

/// Render a scalar value as plain text for an input box.
fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
