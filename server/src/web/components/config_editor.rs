use dioxus::prelude::*;
use dioxus_i18n::t;

use super::config_filter_chips::FilterChips;
use super::config_save_bar::SaveBar;
use super::extra_config_modal::{ExtraConfigField, ExtraConfigModalHost};
use crate::models::ClusterConfig;
#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

#[server]
async fn get_current_config(cluster_id: String) -> Result<Option<ClusterConfig>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
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
    let mut json: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
    mac_mgmt_common::config_migrate::migrate(&mut json);
    // Validate
    let _: mac_mgmt_common::ClusterConfig = serde_json::from_value(json.clone())
        .map_err(|e| ServerFnError::new(format!("invalid config: {e}")))?;

    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
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
    let value = serde_json::to_value(&schema).map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(value)
}

/// Convert a raw secret value into a vault secret and return the `secret:NAME` reference.
#[server]
async fn convert_to_secret(
    cluster_id: String,
    name: String,
    value: String,
) -> Result<String, ServerFnError> {
    use aes_gcm::aead::{Aead, KeyInit, OsRng};
    use aes_gcm::{AeadCore, Aes256Gcm, Key};

    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }

    let cfg = crate::config::config();
    let secrets = cfg.secrets.as_ref().ok_or_else(|| ServerFnError::new("secrets not configured"))?;
    let key_b64 = &secrets.encryption_key;
    let key_bytes =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key_b64)
            .map_err(|e| ServerFnError::new(format!("invalid encryption key: {e}")))?;
    let key = *Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, value.as_bytes())
        .map_err(|_| ServerFnError::new("encryption failed"))?;
    let mut encrypted = nonce.to_vec();
    encrypted.extend_from_slice(&ciphertext);

    // Upsert: create or update the secret
    sqlx::query(
        "INSERT INTO cluster_secrets (cluster_id, name, encrypted_value) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (cluster_id, name) DO UPDATE SET encrypted_value = $3, updated_at = now()",
    )
    .bind(uuid)
    .bind(&name)
    .bind(&encrypted)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncConfig).await;
    Ok(format!("secret:{name}"))
}

/// Generate a random API key and return (raw_key, hex-encoded multihash).
/// The raw key is shown once to the user; only the multihash is stored.
#[server]
async fn generate_ai_proxy_key() -> Result<(String, String), ServerFnError> {
    use rand::RngCore;
    use sha2::{Digest, Sha256};

    const SHA2_256: u64 = 0x12;

    // Generate 32 random bytes, encode as "sk-" prefixed hex for UX.
    let mut key_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut key_bytes);
    let raw_key = format!("sk-{}", hex::encode(key_bytes));

    // SHA2-256 multihash of the raw key string.
    let digest = Sha256::digest(raw_key.as_bytes());
    let mh = multihash::Multihash::<32>::wrap(SHA2_256, &digest)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let key_hash = hex::encode(mh.to_bytes());

    Ok((raw_key, key_hash))
}

/// One filter chip in the page-header strip. Drives which section
/// cards the editor renders.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SectionFilter {
    All,
    Enabled,
    Modified,
    Errors,
}

/// Newtype for the active filter so child components can `use_context`
/// without colliding with other `Signal<SectionFilter>` values.
#[derive(Clone, Copy)]
pub struct ActiveSectionFilter(pub Signal<SectionFilter>);

/// Parsed last-saved snapshot — single source of truth for "is this
/// field modified?" comparisons. Updated whenever `saved_text` changes
/// (initial load, post-save, post-discard). Field rows and the
/// per-section counter both read this baseline; "modified" everywhere
/// in the UI means *diverges from the last saved value*, not *diverges
/// from the schema default*. The two used to be conflated, which made
/// already-saved customizations look unsaved.
#[derive(Clone, Copy)]
pub struct EditorBaseline(pub Signal<serde_json::Value>);

/// Cross-component counters: how many fields are modified, how many
/// sections are touched, and how many sections are currently enabled.
/// Computed once per render in `StructuredEditor` and consumed by both
/// the `SaveBar` toast ("N unsaved changes in M sections") and the
/// `FilterChips` row (chip suffix counts: All 17, Enabled 12, …).
#[derive(Clone, Copy, PartialEq, Default)]
pub struct EditorStats {
    pub modified_fields: Signal<usize>,
    pub modified_sections: Signal<usize>,
    pub total_sections: Signal<usize>,
    pub enabled_sections: Signal<usize>,
    pub error_sections: Signal<usize>,
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
    let mut saved_text = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut initialized = use_signal(|| false);
    let mut raw_mode = use_signal(|| false);
    let filter = use_signal(|| SectionFilter::All);
    use_context_provider(|| ActiveSectionFilter(filter));
    let stats = EditorStats {
        modified_fields: use_signal(|| 0usize),
        modified_sections: use_signal(|| 0usize),
        total_sections: use_signal(|| 0usize),
        enabled_sections: use_signal(|| 0usize),
        error_sections: use_signal(|| 0usize),
    };
    use_context_provider(|| stats);

    // Parsed mirror of `saved_text` — recomputed whenever the saved
    // snapshot changes (initial load, post-save, discard). Provided as
    // context so any field row can compute "am I unsaved?" against it
    // without re-parsing the JSON itself.
    let mut baseline_signal = use_signal(|| serde_json::Value::Null);
    use_effect(move || {
        let text = saved_text.read().clone();
        let parsed = serde_json::from_str::<serde_json::Value>(&text)
            .unwrap_or(serde_json::Value::Null);
        if *baseline_signal.read() != parsed {
            baseline_signal.set(parsed);
        }
    });
    use_context_provider(|| EditorBaseline(baseline_signal));

    if !*initialized.read() {
        match &*config.read() {
            Some(Ok(Some(cfg))) => {
                let pretty =
                    serde_json::to_string_pretty(&cfg.config_json).unwrap_or_default();
                editor_text.set(pretty.clone());
                saved_text.set(pretty);
                initialized.set(true);
            }
            Some(Ok(None)) => {
                // No config yet — initialize with empty object so the editor
                // is usable and the save bar appears once the user edits.
                let empty = serde_json::to_string_pretty(&serde_json::json!({}))
                    .unwrap_or_default();
                editor_text.set(empty.clone());
                saved_text.set(empty);
                initialized.set(true);
            }
            _ => {} // still loading or error
        }
    }

    let cid_save = cluster_id.clone();
    // Cloneable so both the strip Save button and the floating SaveBar
    // can hand-off to the same handler without re-allocating closures.
    let do_save = std::rc::Rc::new(move || {
        let cid = cid_save.clone();
        let text = editor_text.read().clone();
        spawn(async move {
            match save_config(cid, text.clone()).await {
                Ok(()) => {
                    error.set(None);
                    saved_text.set(text);
                    config.restart();
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    });

    let do_discard = move |_| {
        let baseline = saved_text.read().clone();
        editor_text.set(baseline);
    };

    // Diff-aware dirty flag — fast string compare, no JSON re-parse on
    // every keystroke. `saved_text` only mutates after a successful save
    // or a discard, so this stays O(1) on the typical input path.
    let dirty = *editor_text.read() != *saved_text.read() && *initialized.read();

    // Block accidental tab-close / hard-nav while there are unsaved
    // changes. Two-step wiring so we don't churn `window.onbeforeunload`
    // on every keystroke (rebinding the handler from inside an effect
    // that tries to schedule a `document::eval` future caused a
    // wasm-bindgen-futures `_was_scheduled` panic):
    //   1. once on mount — install a permanent `beforeunload` that
    //      reads a global `__configDirty` flag.
    //   2. each render where `dirty` changes — update the flag.
    use_effect(move || {
        spawn(async move {
            let _ = document::eval(
                "if (!window.__configBeforeUnloadInstalled) { \
                   window.__configDirty = false; \
                   window.onbeforeunload = function(e) { \
                     if (!window.__configDirty) return; \
                     e.preventDefault(); e.returnValue = ''; return ''; \
                   }; \
                   window.__configBeforeUnloadInstalled = true; \
                 }"
            ).await;
        });
    });
    use_effect(move || {
        // Reading the signals INSIDE the effect makes Dioxus re-run
        // the effect whenever any of them change.
        let edit = editor_text.read();
        let save = saved_text.read();
        let init = *initialized.read();
        let active = init && *edit != *save;
        let val = if active { "true" } else { "false" };
        spawn(async move {
            let _ = document::eval(&format!("window.__configDirty = {val};")).await;
        });
    });

    let last_saved_at: Option<String> = match &*config.read() {
        Some(Ok(Some(cfg))) => Some(cfg.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
        _ => None,
    };

    rsx! {
        if let Some(err) = &*error.read() {
            p { class: "text-danger text-sm mb-2", "{err}" }
        }

        // Filter / mode strip — under the page header. Mirrors the
        // design's "All / Enabled / Modified / Errors" pill row.
        // The Save action lives entirely in the floating SaveBar at
        // the bottom of the viewport, so the strip stays focused on
        // filtering and view modes.
        div { class: "config-strip",
            FilterChips { filter }
            label { class: "ml-auto text-xs text-fg-muted flex items-center gap-1.5 cursor-pointer select-none",
                input {
                    r#type: "checkbox",
                    checked: *raw_mode.read(),
                    onchange: move |evt| raw_mode.set(evt.checked()),
                }
                {t!("config-editor-raw-json")}
            }
        }

        div {
            if *raw_mode.read() {
                textarea {
                    class: "w-full h-64 font-mono text-sm border border-line rounded-md p-2 mb-2 dark:bg-surface-2 dark:text-fg",
                    placeholder: t!("config-editor-paste-placeholder"),
                    value: "{editor_text}",
                    oninput: move |evt| editor_text.set(evt.value()),
                }
            } else {
                {match &*schema.read() {
                    Some(Ok(schema_val)) => {
                        rsx! {
                            StructuredEditor {
                                cluster_id: cluster_id.clone(),
                                schema: schema_val.clone(),
                                json_text: editor_text,
                            }
                        }
                    }
                    Some(Err(e)) => rsx! {
                        p { class: "text-danger text-sm", {t!("config-editor-schema-error", error: e.to_string())} }
                        textarea {
                            class: "w-full h-64 font-mono text-sm border border-line rounded-md p-2 mb-2 dark:bg-surface-2 dark:text-fg",
                            placeholder: t!("config-editor-paste-placeholder"),
                            value: "{editor_text}",
                            oninput: move |evt| editor_text.set(evt.value()),
                        }
                    },
                    None => rsx! { p { class: "text-sm", {t!("config-editor-loading-schema")} } },
                }}
            }
        }

        // Floating save bar. Visible only when the editor diverges from
        // the last server snapshot. Centred at the bottom of the
        // viewport with brand-orange ring.
        if !read_only && dirty {
            {
                let do_save_bar = std::rc::Rc::clone(&do_save);
                rsx! {
                    SaveBar {
                        last_saved: last_saved_at,
                        editor_text,
                        saved_text,
                        on_save: move |_| do_save_bar(),
                        on_discard: do_discard,
                        stats,
                    }
                }
            }
        }
    }
}

/// Category ordering used by the structured editor.
const CATEGORY_ORDER: &[&str] = &[
    "identity",
    "llm-providers",
    "agents",
    "infra",
    "ops",
    "custom",
];

/// Parsed information about a top-level config section derived from the schema.
struct SectionMeta {
    /// Config key (e.g. "ollama", "cloud")
    name: String,
    /// The raw property schema (may contain $ref, x-category, etc.)
    property_schema: serde_json::Value,
    /// Resolved schema (after following $ref)
    resolved: serde_json::Value,
    /// Category from x-category extension
    #[allow(dead_code)]
    category: String,
    /// Whether this is an array section (Vec<T>)
    is_array: bool,
    /// For array sections, which field to use as the entry label
    array_entry_label: Option<String>,
    /// Whether the section is always-on (no enable/disable toggle)
    always_on: bool,
}

/// Gather section metadata from the schema, grouped by category.
fn gather_sections(
    properties: &serde_json::Map<String, serde_json::Value>,
    defs: &serde_json::Value,
) -> Vec<(String, Vec<SectionMeta>)> {
    let mut by_category: std::collections::HashMap<String, Vec<SectionMeta>> =
        std::collections::HashMap::new();

    for (name, prop_schema) in properties {
        let category = prop_schema
            .get("x-category")
            .and_then(|c| c.as_str())
            .unwrap_or("other")
            .to_string();

        let always_on = prop_schema
            .get("x-always-on")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let array_entry_label = prop_schema
            .get("x-array-entry-label")
            .and_then(|v| v.as_str())
            .map(String::from);

        let resolved = resolve_ref(prop_schema, defs);
        let section_type = resolved
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("object");
        let is_array = section_type == "array";

        by_category
            .entry(category.clone())
            .or_default()
            .push(SectionMeta {
                name: name.clone(),
                property_schema: prop_schema.clone(),
                resolved,
                category,
                is_array,
                array_entry_label,
                always_on,
            });
    }

    // Return in defined order
    let mut result = Vec::new();
    for cat in CATEGORY_ORDER {
        if let Some(sections) = by_category.remove(*cat) {
            result.push((cat.to_string(), sections));
        }
    }
    // Any remaining categories
    for (cat, sections) in by_category {
        result.push((cat, sections));
    }
    result
}

/// Renders structured form sections from JSON Schema, keeping the JSON signal in sync.
/// Sections are grouped by `x-category` from the schema and rendered as collapsible cards.
#[component]
fn StructuredEditor(cluster_id: String, schema: serde_json::Value, json_text: Signal<String>) -> Element {
    let mut form_values: Signal<serde_json::Value> =
        use_signal(|| serde_json::Value::Object(Default::default()));
    let extra_config_open = use_signal(|| false);
    let model_select_request: Signal<Option<super::model_select_modal::ModelSelectRequest>> =
        use_signal(|| None);
    use_context_provider(|| model_select_request);

    // Keep form_values in sync when json_text changes (e.g. after config loads)
    use_effect(move || {
        let text = json_text.read().clone();
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
            form_values.set(parsed);
        }
    });

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

    let categories = gather_sections(&properties, &defs);

    // Build sidebar data: Vec<(category_id, Vec<(section_name, always_on, is_array)>)>
    let sidebar_data: Vec<(String, Vec<(String, bool, bool)>)> = categories
        .iter()
        .map(|(cat_id, sections)| {
            let items: Vec<_> = sections
                .iter()
                .map(|s| (s.name.clone(), s.always_on, s.is_array))
                .collect();
            (cat_id.clone(), items)
        })
        .collect();
    let total_sections: usize = sidebar_data.iter().map(|(_, items)| items.len()).sum();

    // Read the page-level filter chip (All / Enabled / Modified /
    // Errors). Hidden sections are dropped from the main column but
    // STAY in the right rail — clicking a rail entry still expands and
    // scrolls into view, matching the design's "rail is always
    // complete" rule.
    let ActiveSectionFilter(filter_sig) = use_context::<ActiveSectionFilter>();
    let active_filter = *filter_sig.read();
    let form_snapshot = form_values.read().clone();
    let EditorBaseline(baseline_sig) = use_context::<EditorBaseline>();
    let baseline_snapshot = baseline_sig.read().clone();

    // Compute per-section modified counts once per render and publish
    // through `EditorStats` so the floating save bar (rendered in the
    // ConfigEditor parent) can read totals without re-walking the
    // schema. Result is also reused below for the right-rail "N∆" pill.
    //
    // "Modified" means *diverges from the saved snapshot*, not *diverges
    // from the schema default*. Comparing to the saved baseline is what
    // the user expects: after a save the count drops to zero; before a
    // save it reflects exactly the unsaved diff.
    let modified_per_section: std::collections::HashMap<String, usize> = {
        let mut out = std::collections::HashMap::new();
        for (_, sections) in &categories {
            for s in sections {
                if s.is_array {
                    continue;
                }
                let props = s
                    .resolved
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .cloned()
                    .unwrap_or_default();
                let count = props
                    .keys()
                    .filter(|fname| {
                        if fname.as_str() == "enabled" {
                            return false;
                        }
                        let path = [s.name.clone(), fname.to_string()];
                        let current = get_at_path(&form_snapshot, &path);
                        let saved = get_at_path(&baseline_snapshot, &path);
                        current != saved
                    })
                    .count();
                if count > 0 {
                    out.insert(s.name.clone(), count);
                }
            }
        }
        out
    };
    let total_modified_fields: usize = modified_per_section.values().sum();
    let touched_section_count = modified_per_section.len();
    // Enabled count: a section "counts" if it's always_on, is an array,
    // has no `enabled` property, or has `enabled = true` in the form.
    // Mirrors the `SectionFilter::Enabled` logic so the chip and the
    // filter agree on the count.
    let enabled_section_count: usize = {
        let mut count = 0usize;
        for (_, sections) in &categories {
            for s in sections {
                if s.always_on || s.is_array {
                    count += 1;
                    continue;
                }
                let has_enabled = s
                    .resolved
                    .get("properties")
                    .and_then(|p| p.get("enabled"))
                    .is_some();
                if !has_enabled {
                    count += 1;
                    continue;
                }
                if get_at_path(&form_snapshot, &[s.name.clone()])
                    .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
                    .unwrap_or(false)
                {
                    count += 1;
                }
            }
        }
        count
    };
    {
        let stats = use_context::<EditorStats>();
        let mut mf = stats.modified_fields;
        let mut ms = stats.modified_sections;
        let mut ts = stats.total_sections;
        let mut es = stats.enabled_sections;
        if *mf.read() != total_modified_fields {
            mf.set(total_modified_fields);
        }
        if *ms.read() != touched_section_count {
            ms.set(touched_section_count);
        }
        if *ts.read() != total_sections {
            ts.set(total_sections);
        }
        if *es.read() != enabled_section_count {
            es.set(enabled_section_count);
        }
    }
    let section_passes_filter = move |s: &SectionMeta| -> bool {
        match active_filter {
            SectionFilter::All => true,
            SectionFilter::Enabled => {
                if s.always_on || s.is_array {
                    return true;
                }
                let has_enabled = s
                    .resolved
                    .get("properties")
                    .and_then(|p| p.get("enabled"))
                    .is_some();
                if !has_enabled {
                    return true;
                }
                get_at_path(&form_snapshot, &[s.name.clone()])
                    .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
                    .unwrap_or(false)
            }
            SectionFilter::Modified => {
                // Same baseline as `modified_per_section` — a section
                // counts as modified iff at least one of its fields
                // differs from the saved snapshot.
                let props = s
                    .resolved
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .cloned()
                    .unwrap_or_default();
                props.keys().any(|fname| {
                    if fname == "enabled" {
                        return false;
                    }
                    let path = [s.name.clone(), fname.clone()];
                    let current = get_at_path(&form_snapshot, &path);
                    let saved = get_at_path(&baseline_snapshot, &path);
                    current != saved
                })
            }
            SectionFilter::Errors => false, // Wired up when validation surfaces per-section errors.
        }
    };

    rsx! {
        ExtraConfigModalHost {
            open: extra_config_open,
            form_values,
            json_text,
        }
        super::model_select_modal::ModelSelectModal {
            request: model_select_request,
            form_values,
            json_text,
        }
        div { class: "flex gap-6",
            // Main content
            div { class: "flex-1 min-w-0 space-y-8 mb-4",
                {categories.into_iter().filter_map(|(category_id, sections)| {
                    let visible: Vec<SectionMeta> = sections.into_iter().filter(|s| section_passes_filter(s)).collect();
                    if visible.is_empty() {
                        return None;
                    }
                    Some((category_id, visible))
                }).map(|(category_id, sections)| {
                    let cat_i18n_key = format!("category-{category_id}");
                    let enabled_count = sections.iter().filter(|s| {
                        if s.always_on { return true; }
                        if s.is_array { return true; }
                        let resolved = &s.resolved;
                        let has_enabled = resolved.get("properties")
                            .and_then(|p| p.get("enabled"))
                            .is_some();
                        if !has_enabled { return true; }
                        get_at_path(&form_values.read(), &[s.name.clone()])
                            .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
                            .unwrap_or(false)
                    }).count();
                    let total_count = sections.len();

                    rsx! {
                        div { key: "{category_id}",
                            // Category header
                            div { class: "flex items-baseline justify-between mb-3 px-1",
                                h3 { class: "text-[15px] font-semibold text-fg-strong tracking-tight",
                                    {t!(&cat_i18n_key)}
                                }
                                span { class: "text-xs text-fg-muted font-mono",
                                    "{enabled_count}/{total_count} "
                                    {t!("config-editor-enabled-suffix")}
                                }
                            }
                            // Section cards
                            div { class: "space-y-3",
                                {sections.into_iter().map(|section| {
                                    let defs = defs.clone();
                                    let section_key = section.name.clone();
                                    if section.is_array {
                                        let items_schema = section.resolved.get("items")
                                            .map(|s| resolve_ref(s, &defs))
                                            .unwrap_or_default();
                                        let path = vec![section.name.clone()];
                                        let entries: Vec<serde_json::Value> = get_at_path(&form_values.read(), &path)
                                            .and_then(|v| v.as_array().cloned())
                                            .unwrap_or_default();
                                        let entry_label_field = section.array_entry_label.clone().unwrap_or_default();
                                        let section_name = section.name.clone();
                                        rsx! {
                                            div { key: "{section_key}", class: "space-y-3",
                                                {entries.iter().enumerate().map(|(idx, _)| {
                                                    let mut entry_path = vec![section_name.clone()];
                                                    entry_path.push(format!("{idx}"));
                                                    let label = get_at_path(&form_values.read(), &entry_path)
                                                        .and_then(|v| v.get(&entry_label_field).and_then(|p| p.as_str().map(String::from)))
                                                        .unwrap_or_else(|| format!("#{idx}"));
                                                    rsx! {
                                                        ArrayEntrySectionCard {
                                                            key: "{section_name}-{idx}",
                                                            section_name: section_name.clone(),
                                                            entry_index: idx,
                                                            entry_label: label,
                                                            items_schema: items_schema.clone(),
                                                            defs: defs.clone(),
                                                            form_values,
                                                            json_text,
                                                            extra_config_open,
                                                            cluster_id: cluster_id.clone(),
                                                        }
                                                    }
                                                })}
                                                {render_add_entry_button(
                                                    &section.name,
                                                    &items_schema,
                                                    &defs,
                                                    form_values,
                                                    json_text,
                                                )}
                                            }
                                        }
                                    } else {
                                        rsx! {
                                            div { key: "{section_key}",
                                                ObjectSectionCard {
                                                    section_name: section.name.clone(),
                                                    section_schema: section.resolved.clone(),
                                                    property_schema: section.property_schema.clone(),
                                                    always_on: section.always_on,
                                                    defs: defs.clone(),
                                                    form_values,
                                                    json_text,
                                                    extra_config_open,
                                                    cluster_id: cluster_id.clone(),
                                                }
                                            }
                                        }
                                    }
                                })}
                            }
                        }
                    }
                })}
            }
            // Right sidebar — page-local rail with the section index.
            // Stays usable when the filter chip hides sections from the
            // main column: clicking a rail entry still expands the
            // matching card and scrolls into view.
            aside { class: "config-right-rail",
                div { class: "border-b border-line pb-3 mb-3",
                    div { class: "kicker mb-1", {t!("config-editor-on-this-page")} }
                    div { class: "text-sm text-fg-strong font-medium",
                        "{total_sections} "
                        {t!("config-editor-sections-count")}
                    }
                }
                nav { class: "space-y-4",
                    {sidebar_data.into_iter().map(|(cat_id, items)| {
                        let cat_i18n_key = format!("category-{cat_id}");
                        let enabled_count = items.iter().filter(|(name, always_on, is_array)| {
                            if *always_on || *is_array { return true; }
                            let resolved = properties.get(name)
                                .map(|ps| resolve_ref(ps, &defs));
                            let has_enabled_prop = resolved.as_ref()
                                .and_then(|r| r.get("properties").and_then(|p| p.get("enabled")))
                                .is_some();
                            if !has_enabled_prop { return true; }
                            get_at_path(&form_values.read(), &[name.clone()])
                                .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
                                .unwrap_or(false)
                        }).count();
                        let total = items.len();
                        rsx! {
                            div { key: "{cat_id}",
                                div { class: "flex items-center justify-between px-2 mb-1",
                                    span { class: "kicker", {t!(&cat_i18n_key)} }
                                    span { class: "text-[10px] font-mono text-fg-faint",
                                        "{enabled_count}/{total}"
                                    }
                                }
                                {items.into_iter().map(|(name, always_on, is_array)| {
                                    let resolved = properties.get(&name)
                                        .map(|ps| resolve_ref(ps, &defs));
                                    let has_enabled = !always_on && !is_array && resolved.as_ref()
                                        .and_then(|r| r.get("properties").and_then(|p| p.get("enabled")))
                                        .is_some();
                                    let is_enabled = if has_enabled {
                                        get_at_path(&form_values.read(), &[name.clone()])
                                            .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
                                            .unwrap_or(false)
                                    } else {
                                        true
                                    };
                                    let dot_cls = if always_on {
                                        "dot dot-info"
                                    } else if is_enabled {
                                        "dot dot-ok"
                                    } else {
                                        "dot dot-muted"
                                    };
                                    let name_display = name.clone();
                                    let mod_count = modified_per_section.get(&name).copied().unwrap_or(0);
                                    rsx! {
                                        a {
                                            key: "{name}",
                                            href: "#sec-{name}",
                                            class: "flex items-center gap-2 px-2 py-1 rounded-md text-xs font-mono text-fg-muted hover:text-fg-strong hover:bg-surface-2 transition-colors",
                                            span { class: "{dot_cls}" }
                                            span { class: "truncate flex-1", "{name_display}" }
                                            if mod_count > 0 {
                                                span { class: "text-[10px] font-mono text-brand", "{mod_count}∆" }
                                            }
                                        }
                                    }
                                })}
                            }
                        }
                    })}
                }
                // Footer anchors — surface the page-level sections that
                // live below the schema-driven editor (Secrets, History).
                // Without these entries the rail looks like it ends with
                // the schema sections while the page continues underneath.
                div { class: "border-t border-line pt-3 mt-3 space-y-0.5",
                    a {
                        href: "#sec-secrets",
                        class: "flex items-center gap-2 px-2 py-1.5 rounded-md text-xs text-fg-muted hover:text-fg-strong hover:bg-surface-2 transition-colors",
                        span { class: "dot dot-info" }
                        span { class: "truncate flex-1", {t!("secrets-title")} }
                    }
                    a {
                        href: "#sec-config-history",
                        class: "flex items-center gap-2 px-2 py-1.5 rounded-md text-xs text-fg-muted hover:text-fg-strong hover:bg-surface-2 transition-colors",
                        span { class: "dot dot-muted" }
                        span { class: "truncate flex-1", {t!("cluster-detail-tab-config-history")} }
                    }
                }
            }
        }
    }
}

/// A section card for a single object config section (e.g. ollama, relay).
#[component]
fn ObjectSectionCard(
    section_name: String,
    section_schema: serde_json::Value,
    property_schema: serde_json::Value,
    always_on: bool,
    defs: serde_json::Value,
    mut form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    cluster_id: String,
) -> Element {
    let mut expanded = use_signal(|| false);
    let mut show_advanced = use_signal(|| false);

    // First-paint expand decision:
    //   1. URL `?expand=all` → expand everything (used by screenshot
    //      automation; also a handy power-user toggle).
    //   2. Otherwise read sessionStorage so the previous user's
    //      open/closed state for THIS cluster is restored.
    // Both reads happen in JS land via `document::eval` because
    // session/local storage isn't available to WASM directly.
    let cid_init = cluster_id.clone();
    let sn_init = section_name.clone();
    use_effect(move || {
        let cid = cid_init.clone();
        let sn = sn_init.clone();
        spawn(async move {
            let script = format!(
                "try {{ \
                  var u = new URL(window.location.href); \
                  if (u.searchParams.get('expand') === 'all') return '1'; \
                  var k = 'cluster-cfg.' + {cid:?} + '.expanded'; \
                  var v = JSON.parse(sessionStorage.getItem(k) || '[]'); \
                  return v.indexOf({sn:?}) !== -1 ? '1' : '0'; \
                }} catch(e) {{ return '0'; }}",
                cid = cid,
                sn = sn
            );
            if let Ok(val) = document::eval(&script).await {
                if val.as_str() == Some("1") {
                    expanded.set(true);
                }
            }
        });
    });

    let sync_to_json = move || {
        let json = form_values.read().clone();
        let mut text = json_text;
        text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
    };

    // Persistence helpers: cloned once so the inline onclick closure
    // below can capture them by `move` without keeping the surrounding
    // String props on its hot path.
    let cid_persist = cluster_id.clone();
    let sn_persist = section_name.clone();

    let properties = section_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    let has_enabled = properties.contains_key("enabled");
    let is_enabled = if has_enabled {
        get_at_path(&form_values.read(), &[section_name.clone()])
            .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
            .unwrap_or(false)
    } else {
        true
    };

    let description = property_schema
        .get("description")
        .or_else(|| section_schema.get("description"))
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();

    // Count fields that diverge from the saved snapshot — same
    // baseline as `modified_per_section` in `StructuredEditor`. The
    // pill in the section header now reads "N modified" only while
    // there are unsaved edits in this section.
    let EditorBaseline(baseline_sig) = use_context::<EditorBaseline>();
    let baseline_snapshot = baseline_sig.read().clone();
    let form_snapshot_card = form_values.read().clone();
    let mod_count = properties.keys().filter(|fname| {
        if fname.as_str() == "enabled" { return false; }
        let path = [section_name.clone(), fname.to_string()];
        let current = get_at_path(&form_snapshot_card, &path);
        let saved = get_at_path(&baseline_snapshot, &path);
        current != saved
    }).count();

    // Split fields into essential and advanced
    let essential_fields: Vec<_> = properties.iter()
        .filter(|(k, v)| {
            k.as_str() != "enabled"
                && !resolve_ref(v, &defs).get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false)
                && !v.get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let advanced_fields: Vec<_> = properties.iter()
        .filter(|(k, v)| {
            k.as_str() != "enabled"
                && (resolve_ref(v, &defs).get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false)
                    || v.get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let dot_class = if always_on {
        "dot dot-info"
    } else if is_enabled {
        "dot dot-ok"
    } else {
        "dot dot-muted"
    };

    // Body visibility is decoupled from the `enabled` toggle: users
    // need to inspect / edit the fields of disabled sections without
    // having to flip the toggle on first (and risk mutating dependent
    // services). The toggle only persists the `enabled` field;
    // everything else stays exactly as it was.
    let show_body = *expanded.read();
    let section_name_toggle = section_name.clone();
    let section_name_display = section_name.clone();
    let section_anchor = format!("sec-{section_name}");

    rsx! {
        div { class: "card overflow-hidden scroll-mt-20", id: "{section_anchor}",
            // Header
            div {
                class: "flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-surface-2 transition-colors",
                onclick: move |_| {
                    let new_val = !*expanded.read();
                    expanded.set(new_val);
                    // Persist the new state so navigating away and
                    // back restores the open / closed sections.
                    let cid = cid_persist.clone();
                    let sn = sn_persist.clone();
                    let nv = if new_val { "true" } else { "false" };
                    spawn(async move {
                        let _ = document::eval(&format!(
                            "try {{ \
                              var k = 'cluster-cfg.' + {cid:?} + '.expanded'; \
                              var v; try {{ v = JSON.parse(sessionStorage.getItem(k) || '[]'); }} catch(e) {{ v = []; }} \
                              var idx = v.indexOf({sn:?}); \
                              if ({nv} && idx === -1) v.push({sn:?}); \
                              if (!{nv} && idx !== -1) v.splice(idx, 1); \
                              sessionStorage.setItem(k, JSON.stringify(v)); \
                            }} catch(e) {{}}",
                            cid = cid, sn = sn, nv = nv,
                        )).await;
                    });
                },
                // Chevron
                span { class: if *expanded.read() { "text-fg-faint text-[10px] font-mono transition-transform rotate-90" } else { "text-fg-faint text-[10px] font-mono transition-transform" },
                    "▶"
                }
                // Status dot
                span { class: "{dot_class}" }
                // Name + description
                div { class: "min-w-0 flex-1",
                    div { class: "flex items-center gap-2 mb-0.5",
                        span { class: "text-sm font-semibold font-mono tracking-tight text-fg-strong",
                            "{section_name_display}"
                        }
                        if mod_count > 0 {
                            span { class: "pill pill-accent text-[10px]",
                                "{mod_count} modified"
                            }
                        }
                    }
                    if !description.is_empty() {
                        div { class: "text-xs text-fg-muted font-mono truncate",
                            "{description}"
                        }
                    }
                }
                // Enable/disable toggle
                if has_enabled && !always_on {
                    div {
                        onclick: move |evt| evt.stop_propagation(),
                        label { class: "relative inline-flex items-center cursor-pointer",
                            input {
                                r#type: "checkbox",
                                class: "sr-only peer",
                                checked: is_enabled,
                                onchange: {
                                    let sn = section_name_toggle.clone();
                                    move |evt| {
                                        set_at_path(
                                            &mut form_values,
                                            &[sn.clone(), "enabled".to_string()],
                                            serde_json::Value::Bool(evt.checked()),
                                        );
                                        sync_to_json();
                                    }
                                },
                            }
                            div { class: "w-8 h-[18px] bg-surface-3 peer-focus-visible:ring-2 peer-focus-visible:ring-brand rounded-full peer peer-checked:bg-brand transition-colors" }
                            div { class: "absolute left-[2px] top-[2px] w-[14px] h-[14px] bg-white rounded-full shadow transition-transform peer-checked:translate-x-[14px]" }
                        }
                    }
                }
            }
            // Body
            if show_body {
                div { class: "border-t border-line",
                    // Essential fields
                    {essential_fields.iter().map(|(field_name, field_schema)| {
                        rsx! {
                            SectionFieldRow {
                                key: "{section_name}-{field_name}",
                                section_name: section_name.clone(),
                                field_name: field_name.clone(),
                                field_schema: field_schema.clone(),
                                defs: defs.clone(),
                                form_values,
                                json_text,
                                extra_config_open,
                                cluster_id: cluster_id.clone(),
                            }
                        }
                    })}
                    // Advanced fields accordion
                    if !advanced_fields.is_empty() {
                        div { class: "border-t border-line",
                            button {
                                class: "w-full text-left px-4 py-2 text-xs text-fg-muted font-medium hover:bg-surface-2 transition-colors flex items-center gap-2",
                                onclick: move |_| { let v = *show_advanced.read(); show_advanced.set(!v); },
                                span { class: if *show_advanced.read() { "text-[9px] font-mono transition-transform rotate-90" } else { "text-[9px] font-mono transition-transform" },
                                    "▶"
                                }
                                {t!("config-editor-advanced", count: advanced_fields.len())}
                            }
                            if *show_advanced.read() {
                                {advanced_fields.iter().map(|(field_name, field_schema)| {
                                    rsx! {
                                        SectionFieldRow {
                                            key: "{section_name}-adv-{field_name}",
                                            section_name: section_name.clone(),
                                            field_name: field_name.clone(),
                                            field_schema: field_schema.clone(),
                                            defs: defs.clone(),
                                            form_values,
                                            json_text,
                                            extra_config_open,
                                            cluster_id: cluster_id.clone(),
                                        }
                                    }
                                })}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A section card for a single entry in an array section (e.g. one cloud provider).
#[component]
fn ArrayEntrySectionCard(
    section_name: String,
    entry_index: usize,
    entry_label: String,
    items_schema: serde_json::Value,
    defs: serde_json::Value,
    mut form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    cluster_id: String,
) -> Element {
    let mut expanded = use_signal(|| false);
    let mut show_advanced = use_signal(|| false);

    let sync_to_json = move || {
        let json = form_values.read().clone();
        let mut text = json_text;
        text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
    };

    let properties = items_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    let has_enabled = properties.contains_key("enabled");
    let entry_path = vec![section_name.clone(), format!("{entry_index}")];
    let is_enabled = if has_enabled {
        get_at_path(&form_values.read(), &entry_path)
            .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
            .unwrap_or(true)
    } else {
        true
    };

    let essential_fields: Vec<_> = properties.iter()
        .filter(|(k, v)| {
            k.as_str() != "enabled"
                && !resolve_ref(v, &defs).get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false)
                && !v.get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false)
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let advanced_fields: Vec<_> = properties.iter()
        .filter(|(k, v)| {
            k.as_str() != "enabled"
                && (resolve_ref(v, &defs).get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false)
                    || v.get("x-advanced").and_then(|v| v.as_bool()).unwrap_or(false))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let dot_class = if is_enabled { "dot dot-ok" } else { "dot dot-muted" };
    // Same rule as in `ObjectSectionCard`: expand state is independent
    // of the enabled toggle so users can read disabled sections.
    let show_body = *expanded.read();
    let section_name_toggle = section_name.clone();
    let path_remove = vec![section_name.clone()];
    let sync_remove = sync_to_json.clone();
    let entry_anchor = format!("sec-{section_name}-{entry_index}");

    rsx! {
        div { class: "card overflow-hidden scroll-mt-20", id: "{entry_anchor}",
            // Header
            div {
                class: "flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-surface-2 transition-colors",
                onclick: move |_| { let v = *expanded.read(); expanded.set(!v); },
                span { class: if *expanded.read() { "text-fg-faint text-[10px] font-mono transition-transform rotate-90" } else { "text-fg-faint text-[10px] font-mono transition-transform" },
                    "▶"
                }
                span { class: "{dot_class}" }
                div { class: "min-w-0 flex-1",
                    div { class: "flex items-center gap-2 mb-0.5",
                        span { class: "text-sm font-semibold font-mono tracking-tight text-fg-strong",
                            "{entry_label}"
                        }
                        span { class: "kicker", "{section_name}" }
                    }
                }
                // Enable/disable toggle
                if has_enabled {
                    div {
                        onclick: move |evt| evt.stop_propagation(),
                        label { class: "relative inline-flex items-center cursor-pointer",
                            input {
                                r#type: "checkbox",
                                class: "sr-only peer",
                                checked: is_enabled,
                                onchange: {
                                    let sn = section_name_toggle.clone();
                                    let idx = entry_index;
                                    move |evt| {
                                        set_at_path(
                                            &mut form_values,
                                            &[sn.clone(), format!("{idx}"), "enabled".to_string()],
                                            serde_json::Value::Bool(evt.checked()),
                                        );
                                        sync_to_json();
                                    }
                                },
                            }
                            div { class: "w-8 h-[18px] bg-surface-3 peer-focus-visible:ring-2 peer-focus-visible:ring-brand rounded-full peer peer-checked:bg-brand transition-colors" }
                            div { class: "absolute left-[2px] top-[2px] w-[14px] h-[14px] bg-white rounded-full shadow transition-transform peer-checked:translate-x-[14px]" }
                        }
                    }
                }
                // Remove button
                div {
                    onclick: move |evt| evt.stop_propagation(),
                    button {
                        class: "btn btn-xs btn-danger-soft",
                        r#type: "button",
                        onclick: {
                            let fp = path_remove.clone();
                            move |_| {
                                let mut arr = get_at_path(&form_values.read(), &fp)
                                    .and_then(|v| v.as_array().cloned())
                                    .unwrap_or_default();
                                if entry_index < arr.len() {
                                    arr.remove(entry_index);
                                }
                                set_at_path(&mut form_values, &fp, serde_json::Value::Array(arr));
                                sync_remove();
                            }
                        },
                        {t!("config-editor-remove")}
                    }
                }
            }
            // Body
            if show_body {
                div { class: "border-t border-line",
                    {essential_fields.iter().map(|(field_name, field_schema)| {
                        let base_path = format!("{section_name}.{entry_index}");
                        rsx! {
                            SectionFieldRow {
                                key: "{base_path}-{field_name}",
                                section_name: base_path.clone(),
                                field_name: field_name.clone(),
                                field_schema: field_schema.clone(),
                                defs: defs.clone(),
                                form_values,
                                json_text,
                                extra_config_open,
                                cluster_id: cluster_id.clone(),
                            }
                        }
                    })}
                    if !advanced_fields.is_empty() {
                        div { class: "border-t border-line",
                            button {
                                class: "w-full text-left px-4 py-2 text-xs text-fg-muted font-medium hover:bg-surface-2 transition-colors flex items-center gap-2",
                                onclick: move |_| { let v = *show_advanced.read(); show_advanced.set(!v); },
                                span { class: if *show_advanced.read() { "text-[9px] font-mono transition-transform rotate-90" } else { "text-[9px] font-mono transition-transform" },
                                    "▶"
                                }
                                {t!("config-editor-advanced", count: advanced_fields.len())}
                            }
                            if *show_advanced.read() {
                                {advanced_fields.iter().map(|(field_name, field_schema)| {
                                    let base_path = format!("{section_name}.{entry_index}");
                                    rsx! {
                                        SectionFieldRow {
                                            key: "{base_path}-adv-{field_name}",
                                            section_name: base_path.clone(),
                                            field_name: field_name.clone(),
                                            field_schema: field_schema.clone(),
                                            defs: defs.clone(),
                                            form_values,
                                            json_text,
                                            extra_config_open,
                                            cluster_id: cluster_id.clone(),
                                        }
                                    }
                                })}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Render the "+ Add entry" button for array sections. The label is
/// per-section so the affordance reads naturally — "+ Add cloud LLM
/// provider" instead of the generic "+ Add entry". Section names that
/// don't have a specific label fall back to the generic copy.
fn render_add_entry_button(
    section_name: &str,
    items_schema: &serde_json::Value,
    defs: &serde_json::Value,
    mut form_values: Signal<serde_json::Value>,
    mut json_text: Signal<String>,
) -> Element {
    let path = vec![section_name.to_string()];
    let items_schema = items_schema.clone();
    let defs = defs.clone();
    let label = match section_name {
        "cloud" => t!("config-add-cloud"),
        "custom-service" => t!("config-add-custom-service"),
        _ => t!("config-editor-add-entry"),
    };
    rsx! {
        button {
            r#type: "button",
            class: "btn btn-sm btn-ghost w-full border border-dashed border-line text-fg-muted hover:text-brand hover:border-brand transition-colors",
            onclick: move |_| {
                let mut arr = get_at_path(&form_values.read(), &path)
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default();
                let new_obj = build_default_object(&items_schema, &defs);
                arr.push(new_obj);
                set_at_path(&mut form_values, &path, serde_json::Value::Array(arr));
                json_text.set(serde_json::to_string_pretty(&*form_values.read()).unwrap_or_default());
            },
            "{label}"
        }
    }
}

/// A single field row within a section card. This is a component so hooks are safe.
#[component]
fn SectionFieldRow(
    /// Dot-separated path prefix (e.g. "ollama" or "cloud.0")
    section_name: String,
    field_name: String,
    field_schema: serde_json::Value,
    defs: serde_json::Value,
    mut form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    cluster_id: String,
) -> Element {
    let resolved = resolve_ref(&field_schema, &defs);
    let is_secret = resolved.get("x-secret").and_then(|v| v.as_bool()).unwrap_or(false)
        || field_schema.get("x-secret").and_then(|v| v.as_bool()).unwrap_or(false);
    let model_source_kind = resolved
        .get("x-model-source")
        .or_else(|| field_schema.get("x-model-source"))
        .and_then(|v| v.as_str())
        .map(String::from);
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

    let is_object = field_type == "object" || resolved.get("properties").is_some();

    // Build the path segments from the dot-separated section_name + field_name
    let mut field_path: Vec<String> = section_name.split('.').map(String::from).collect();
    field_path.push(field_name.clone());

    let current_value = get_at_path(&form_values.read(), &field_path);
    let schema_default = resolved.get("default").cloned();

    // Two orthogonal flags drive the field-row chrome:
    //   * `is_modified` — diverges from the *saved* snapshot. Drives
    //     the brand dot next to the label and contributes to the
    //     section / page modified counters.
    //   * `differs_from_default` — diverges from the *schema default*.
    //     Drives the reset-to-default button. A field can be saved-but-
    //     customized (no dot, but reset is still useful).
    let EditorBaseline(baseline_sig) = use_context::<EditorBaseline>();
    let saved_value = get_at_path(&baseline_sig.read(), &field_path);
    let is_modified = current_value != saved_value;
    let differs_from_default = current_value.as_ref().is_some_and(|v| {
        schema_default.as_ref().map_or(true, |d| v != d)
    });

    let sync = move || {
        let json = form_values.read().clone();
        let mut text = json_text;
        text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
    };

    // Special-case: extra_config under openclaw — render in the same
    // row layout as every other field so it visually aligns with the
    // gateway / skills / telegram sub-cards below. Without the wrapper
    // it sat flush against the openclaw card edges with its own
    // bespoke alignment, which made the section feel "two-headed".
    if field_name == "extra_config" && section_name.contains("openclaw") {
        return rsx! {
            div { class: "px-4 py-3 border-t border-line",
                div { class: "grid grid-cols-[1fr_28px] gap-2 items-start sm:grid-cols-[200px_1fr_28px] sm:gap-3",
                    div { class: "pt-1.5",
                        div { class: "flex items-center gap-1.5",
                            span { class: "text-xs font-medium font-mono text-fg-strong", "{field_name}" }
                        }
                        if !description.is_empty() {
                            p { class: "text-[11px] text-fg-faint mt-0.5 leading-tight", "{description}" }
                        }
                    }
                    div { class: "min-w-0 col-span-2 sm:col-span-1",
                        ExtraConfigField {
                            form_values,
                            open: extra_config_open,
                        }
                    }
                    div { class: "flex justify-end pt-1" }
                }
            }
        };
    }

    // Special-case: key_hash under ai_proxy.keys
    if field_name == "key_hash" && section_name.contains("keys") {
        return rsx! {
            div { class: "px-4 py-3 border-t border-line",
                KeyHashField {
                    field_path,
                    form_values,
                    json_text,
                }
            }
        };
    }

    if is_object {
        // Render as a nested sub-card. Tinted surface + rounded edges
        // give subsections (gateway / skills / telegram inside openclaw)
        // their own visual container without re-using the heavy outer
        // section card chrome — keeps the hierarchy readable when an
        // agent has many subsections. Sub-card fields are routed
        // through `SectionFieldRow` (the same renderer used for
        // top-level fields) so the "label · input · reset" 3-column
        // grid is consistent at every depth — gateway.host now lines
        // up with daemon.health_interval rather than stacking
        // label-on-top via the legacy flat layout.
        let nested_section = field_path.join(".");
        let child_props = resolved
            .get("properties")
            .and_then(|p| p.as_object())
            .cloned()
            .unwrap_or_default();
        let defs_clone = defs.clone();
        return rsx! {
            div { class: "border-t border-line",
                div { class: "m-3 rounded-lg bg-surface-2/60 border border-line-soft overflow-hidden",
                    div { class: "px-4 py-2.5 border-b border-line-soft bg-surface-2/40",
                        div { class: "flex items-center gap-2",
                            span { class: "kicker text-info", "{field_name}" }
                        }
                        if !description.is_empty() {
                            p { class: "text-xs text-fg-muted mt-0.5 leading-snug", "{description}" }
                        }
                    }
                    div {
                        for (cname, cschema) in child_props.into_iter() {
                            {
                                let key = format!("{nested_section}.{cname}");
                                rsx! {
                                    SectionFieldRow {
                                        key: "{key}",
                                        section_name: nested_section.clone(),
                                        field_name: cname,
                                        field_schema: cschema,
                                        defs: defs_clone.clone(),
                                        form_values,
                                        json_text,
                                        extra_config_open,
                                        cluster_id: cluster_id.clone(),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };
    }

    let fp = field_path.clone();
    let fp2 = field_path.clone();
    let reset_path = field_path.clone();
    let reset_default = schema_default.clone();
    let sync_reset = sync.clone();

    rsx! {
        div { class: "px-4 py-3 border-t border-line",
            // Two-track layout on phones (label on top, input below) so
            // the schema's verbose snake_case field names don't get
            // crushed into a 200px column. Switches to the desktop
            // 3-column grid at `sm+`.
            div { class: "grid grid-cols-[1fr_28px] gap-2 items-start sm:grid-cols-[200px_1fr_28px] sm:gap-3",
                // Label column
                div { class: if field_type == "boolean" { "pt-0" } else { "pt-1.5" },
                    div { class: "flex items-center gap-1.5",
                        span { class: "text-xs font-medium font-mono text-fg-strong", "{field_name}" }
                        if is_modified {
                            span { class: "w-1.5 h-1.5 rounded-full bg-brand", title: "unsaved" }
                        }
                    }
                    if !description.is_empty() {
                        p { class: "text-[11px] text-fg-faint mt-0.5 leading-tight", "{description}" }
                    }
                }
                // Input column. On phones it spans both grid columns
                // (under the label) so the field gets the full row
                // width; on `sm+` it sits as the middle column.
                div { class: "min-w-0 col-span-2 sm:col-span-1 space-y-1",
                    {render_field_input(
                        &field_type,
                        &resolved,
                        &defs,
                        is_secret,
                        current_value.clone(),
                        schema_default.as_ref(),
                        &description,
                        &field_name,
                        fp.clone(),
                        fp2.clone(),
                        form_values,
                        baseline_sig,
                        json_text,
                        extra_config_open,
                        cluster_id.clone(),
                        sync.clone(),
                        field_name.clone(),
                    )}
                    // "Select models" button for fields with x-model-source
                    if let Some(ref source_kind) = model_source_kind {
                        {
                            let source_kind = source_kind.clone();
                            let is_multi = field_type == "array";
                            let field_path_for_modal = field_path.clone();
                            let section = section_name.clone();
                            let mut model_select_req = use_context::<Signal<Option<super::model_select_modal::ModelSelectRequest>>>();
                            rsx! {
                                button {
                                    r#type: "button",
                                    class: "btn btn-xs btn-secondary",
                                    onclick: move |_| {
                                        // Read current value from form for pre-selection
                                        let current = {
                                            let fv = form_values.read();
                                            let val = get_at_path(&fv, &field_path_for_modal);
                                            match val {
                                                Some(serde_json::Value::Array(arr)) => {
                                                    arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                                                }
                                                Some(serde_json::Value::String(s)) => {
                                                    if s.is_empty() { vec![] } else { vec![s] }
                                                }
                                                _ => vec![],
                                            }
                                        };
                                        // For cloud entries, read provider/api_key from sibling fields
                                        let (provider, base_url, api_key) = if source_kind == "cloud" {
                                            let fv = form_values.read();
                                            // section_name is like "cloud.0"
                                            let parts: Vec<String> = section.split('.').map(String::from).collect();
                                            let entry = get_at_path(&fv, &parts);
                                            let prov = entry.as_ref()
                                                .and_then(|e| e.get("provider"))
                                                .and_then(|p| p.as_str())
                                                .map(String::from);
                                            let bu = entry.as_ref()
                                                .and_then(|e| e.get("base_url"))
                                                .and_then(|b| b.as_str())
                                                .and_then(|s| if s.is_empty() { None } else { Some(s) })
                                                .or_else(|| {
                                                    prov.as_deref().and_then(|p| {
                                                        // Use CloudProvider base URL
                                                        match p {
                                                            "Anthropic" => Some("https://api.anthropic.com/v1"),
                                                            "Openai" => Some("https://api.openai.com/v1"),
                                                            "Google" => Some("https://generativelanguage.googleapis.com/v1beta"),
                                                            "Mistral" => Some("https://api.mistral.ai/v1"),
                                                            "Groq" => Some("https://api.groq.com/openai/v1"),
                                                            "Xai" => Some("https://api.x.ai/v1"),
                                                            "Deepseek" => Some("https://api.deepseek.com/v1"),
                                                            "Openrouter" => Some("https://openrouter.ai/api/v1"),
                                                            "Together" => Some("https://api.together.xyz/v1"),
                                                            _ => None,
                                                        }
                                                    })
                                                })
                                                .map(String::from);
                                            let ak = entry.as_ref()
                                                .and_then(|e| e.get("api_key"))
                                                .and_then(|k| k.as_str())
                                                .map(String::from);
                                            (prov.map(|p| p.to_lowercase()), bu, ak)
                                        } else {
                                            (None, None, None)
                                        };
                                        model_select_req.set(Some(super::model_select_modal::ModelSelectRequest {
                                            source_kind: source_kind.clone(),
                                            current,
                                            multi: is_multi,
                                            field_path: field_path_for_modal.clone(),
                                            provider,
                                            base_url,
                                            api_key,
                                            gateway_host: None,
                                            gateway_port: None,
                                        }));
                                    },
                                    {t!("model-select-browse")}
                                }
                            }
                        }
                    }
                }
                // Reset column. The arrow does the *most useful* thing
                // for the field's current state:
                //   * if the field has unsaved edits — discard them by
                //     reverting to the saved baseline value;
                //   * else (saved-but-customized) — set the schema
                //     default so the next save wipes the customization.
                //
                // The first branch fixes a sharp edge: schemars emits
                // `null` as the default for `Option<T>` fields, so the
                // legacy "set schema default" reset would write
                // `field: null` into form_values when the saved JSON
                // had the field absent — leaving the editor permanently
                // dirty even though the user's intent was "undo".
                div { class: "flex justify-end pt-1",
                    if is_modified || differs_from_default {
                        button {
                            r#type: "button",
                            class: "text-fg-faint hover:text-danger text-xs",
                            title: if is_modified {
                                t!("config-editor-discard-field")
                            } else {
                                t!("config-editor-reset-default")
                            },
                            onclick: move |evt| {
                                evt.prevent_default();
                                evt.stop_propagation();
                                if is_modified {
                                    let baseline = baseline_sig.read().clone();
                                    revert_field_to_saved(&mut form_values, &baseline, &reset_path);
                                } else if let Some(ref def) = reset_default {
                                    set_at_path(&mut form_values, &reset_path, def.clone());
                                } else {
                                    remove_at_path(&mut form_values, &reset_path);
                                }
                                sync_reset();
                            },
                            "↺"
                        }
                    }
                }
            }
        }
    }
}

/// Render the appropriate input widget for a field based on its schema type.
///
/// `schema_default` is used as a visual fall-back when the user hasn't set
/// a value yet: text/number inputs show it as a `placeholder`, booleans
/// flip ON when the schema default is `true`, and selects pre-highlight
/// the matching option. The form state itself stays empty, so saving a
/// pristine field keeps the JSON sparse — serde will re-apply the default
/// on read either way.
///
/// When the schema has no `default`, the placeholder falls back to the
/// field's `description` (truncated) so every input gives the user *some*
/// hint about what to enter — addressing the "every field needs a
/// placeholder" UX request.
#[allow(clippy::too_many_arguments)]
fn render_field_input(
    field_type: &str,
    resolved: &serde_json::Value,
    defs: &serde_json::Value,
    is_secret: bool,
    current_value: Option<serde_json::Value>,
    schema_default: Option<&serde_json::Value>,
    description: &str,
    field_label: &str,
    fp: Vec<String>,
    fp2: Vec<String>,
    mut form_values: Signal<serde_json::Value>,
    baseline: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    cluster_id: String,
    sync: impl Fn() + Clone + 'static,
    field_name: String,
) -> Element {
    // Build a fallback placeholder for fields without a schema default
    // — see `placeholder_fallback`. Computed once here so each match
    // arm can reuse it without re-parsing the description.
    let description_placeholder = placeholder_fallback(description, field_label);
    match field_type {
        "boolean" => {
            // Visual fall-back: when the field hasn't been explicitly set,
            // surface the schema default so toggles look correct on a
            // sparse config. Serde would apply the same default on read,
            // so the toggle's "off without user input" state is misleading.
            let checked = current_value
                .as_ref()
                .and_then(|v| v.as_bool())
                .or_else(|| schema_default.and_then(|d| d.as_bool()))
                .unwrap_or(false);
            let sync_c = sync.clone();
            rsx! {
                label { class: "relative inline-flex items-center cursor-pointer",
                    input {
                        r#type: "checkbox",
                        class: "sr-only peer",
                        checked: checked,
                        onchange: move |evt| {
                            set_at_path(&mut form_values, &fp,
                                serde_json::Value::Bool(evt.checked()));
                            sync_c();
                        },
                    }
                    div { class: "w-8 h-[18px] bg-surface-3 peer-focus-visible:ring-2 peer-focus-visible:ring-brand rounded-full peer peer-checked:bg-brand transition-colors" }
                    div { class: "absolute left-[2px] top-[2px] w-[14px] h-[14px] bg-white rounded-full shadow transition-transform peer-checked:translate-x-[14px]" }
                }
            }
        }
        "integer" => {
            let val_str = current_value
                .as_ref()
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            let placeholder_str = schema_default
                .and_then(|d| d.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_else(|| description_placeholder.clone());
            let sync_c = sync.clone();
            let fp_clear = fp.clone();
            rsx! {
                input {
                    r#type: "number",
                    class: "input input-sm font-mono",
                    value: val_str,
                    placeholder: placeholder_str,
                    oninput: move |evt| {
                        let v = evt.value();
                        if v.is_empty() {
                            remove_at_path(&mut form_values, &fp_clear);
                            // Walk up: prune any parent object the
                            // user implicitly created while typing
                            // that's now empty *and* absent in the
                            // saved baseline. Keeps "type then
                            // backspace" idempotent.
                            let bsl = baseline.read().clone();
                            cleanup_empty_parents(&mut form_values, &bsl, &fp_clear);
                            sync_c();
                        } else if let Ok(n) = v.parse::<i64>() {
                            set_at_path(&mut form_values, &fp,
                                serde_json::json!(n));
                            sync_c();
                        }
                    },
                }
            }
        }
        "array" => {
            let items_schema = resolved.get("items")
                .map(|s| resolve_ref(s, defs))
                .unwrap_or_default();
            let is_object_array = items_schema.get("properties").is_some();

            if is_object_array {
                let entries: Vec<serde_json::Value> = current_value
                    .as_ref()
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let fp_add = fp.clone();
                let sync_add = sync.clone();
                let items_schema_add = items_schema.clone();
                let defs_add = defs.clone();
                rsx! {
                    div { class: "space-y-2",
                        for (idx, _entry) in entries.iter().enumerate() {
                            {
                                let items_c = items_schema.clone();
                                let defs_c = defs.clone();
                                let fp_r = fp.clone();
                                let sync_r = sync.clone();
                                let mut entry_path = fp.clone();
                                entry_path.push(format!("{idx}"));
                                rsx! {
                                    div { class: "border border-line rounded p-2",
                                        key: "{idx}",
                                        div { class: "flex justify-between items-center mb-1",
                                            span { class: "text-xs font-semibold text-fg-muted", "#{idx}" }
                                            button {
                                                class: "btn btn-xs btn-danger-soft",
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
                                                {t!("config-editor-remove")}
                                            }
                                        }
                                        {render_object_array_entry(
                                            &items_c,
                                            &defs_c,
                                            entry_path,
                                            form_values,
                                            json_text,
                                            extra_config_open,
                                            cluster_id.clone(),
                                            sync.clone(),
                                        )}
                                    }
                                }
                            }
                        }
                        button {
                            r#type: "button",
                            class: "btn btn-xs btn-success-soft",
                            onclick: move |_| {
                                let mut arr = get_at_path(&form_values.read(), &fp_add)
                                    .and_then(|v| v.as_array().cloned())
                                    .unwrap_or_default();
                                let new_obj = build_default_object(&items_schema_add, &defs_add);
                                arr.push(new_obj);
                                set_at_path(&mut form_values, &fp_add,
                                    serde_json::Value::Array(arr));
                                sync_add();
                            },
                            {t!("config-editor-add-entry")}
                        }
                    }
                }
            } else {
                // Array of primitives
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
                let mut new_val = use_signal(String::new);
                rsx! {
                    div { class: "space-y-1",
                        for (idx, item) in items.iter().enumerate() {
                            div {
                                key: "{idx}",
                                class: "flex items-center gap-1",
                                span { class: "flex-1 text-sm font-mono bg-surface-2 border border-line-soft rounded-md px-2 py-0.5 truncate",
                                    "{item}"
                                }
                                button {
                                    class: "link-danger text-xs px-1",
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
                                    "×"
                                }
                            }
                        }
                        div { class: "flex gap-1",
                            input {
                                r#type: "text",
                                class: "input input-sm font-mono flex-1",
                                placeholder: t!("config-editor-add-item"),
                                value: "{new_val}",
                                oninput: move |e| new_val.set(e.value()),
                                onkeypress: {
                                    let fp = fp_add.clone();
                                    let sync_c = sync_add.clone();
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
                                class: "btn btn-xs btn-primary",
                                onclick: {
                                    let fp = fp_add.clone();
                                    let sync_c = sync_add.clone();
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
            let default_str = schema_default
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let sync_c = sync.clone();
            if !enum_values.is_empty() {
                // For selects, fall back to the default option when the
                // form value is unset, so the user sees the active default
                // rather than a meaningless "Select…" prompt.
                let effective_val = if val_str.is_empty() && !default_str.is_empty() {
                    default_str.clone()
                } else {
                    val_str.clone()
                };
                let selected_val = effective_val.clone();
                rsx! {
                    div { class: "relative",
                        select {
                            class: "input input-sm font-mono appearance-none pr-8",
                            value: effective_val,
                            onchange: move |evt| {
                                set_at_path(&mut form_values, &fp,
                                    serde_json::Value::String(evt.value()));
                                sync_c();
                            },
                            if default_str.is_empty() {
                                option { value: "", selected: selected_val.is_empty(), {t!("config-editor-select")} }
                            }
                            {enum_values.iter().map(|v| {
                                let is_selected = *v == selected_val;
                                let v = v.clone();
                                rsx! { option { value: "{v}", selected: is_selected, "{v}" } }
                            })}
                        }
                        span { class: "absolute right-2 top-1/2 -translate-y-1/2 text-fg-faint text-[10px] pointer-events-none",
                            "▾"
                        }
                    }
                }
            } else if is_secret {
                let field_path = fp.clone();
                let secret_key = field_path.join(".");
                rsx! {
                    SecretField {
                        key: "{secret_key}",
                        cluster_id,
                        field_name,
                        field_path,
                        form_values,
                        json_text,
                    }
                }
            } else {
                let placeholder_text = if !default_str.is_empty() {
                    default_str.clone()
                } else {
                    description_placeholder.clone()
                };
                let fp_clear = fp2.clone();
                rsx! {
                    input {
                        r#type: "text",
                        class: "input input-sm font-mono",
                        value: val_str,
                        placeholder: placeholder_text,
                        oninput: move |evt| {
                            let v = evt.value();
                            if v.is_empty() {
                                remove_at_path(&mut form_values, &fp_clear);
                                // Same cleanup as the integer branch:
                                // a typed-then-backspaced text field
                                // shouldn't leave its parent object
                                // dangling as `{}` if the saved JSON
                                // didn't have it.
                                let bsl = baseline.read().clone();
                                cleanup_empty_parents(&mut form_values, &bsl, &fp_clear);
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
    }
}

/// Resolve a `$ref` pointer in the schema, including `anyOf` wrappers
/// from `Option<T>` which schemars generates as `anyOf: [{$ref: ...}, {type: "null"}]`.
/// Dedicated component for the `key_hash` field so `use_signal` is safe
/// (hooks cannot be called inside iterator closures).
#[component]
fn KeyHashField(
    field_path: Vec<String>,
    mut form_values: Signal<serde_json::Value>,
    mut json_text: Signal<String>,
) -> Element {
    let mut generated_key = use_signal(|| None::<String>);
    let fp2 = field_path.clone();
    let fp_gen = field_path.clone();

    let mut sync = move || {
        let json = form_values.read().clone();
        json_text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
    };

    let val_str = get_at_path(&form_values.read(), &field_path)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    rsx! {
        div { class: "flex flex-col gap-0.5",
            label { class: "text-sm font-medium text-fg-strong", {t!("config-editor-key-hash")} }
            p { class: "text-xs text-fg-muted",
                {t!("config-editor-key-hash-help")}
            }
            div { class: "flex gap-2",
                input {
                    r#type: "text",
                    class: "flex-1 border border-line rounded-md px-2 dark:bg-surface-2 dark:text-fg py-1 text-sm font-mono",
                    value: val_str,
                    oninput: move |evt| {
                        let v = evt.value();
                        if v.is_empty() {
                            remove_at_path(&mut form_values, &fp2);
                        } else {
                            set_at_path(&mut form_values, &fp2,
                                serde_json::Value::String(v));
                        }
                        sync();
                    },
                }
                button {
                    r#type: "button",
                    class: "btn btn-md btn-primary whitespace-nowrap",
                    onclick: move |evt| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        let fp = fp_gen.clone();
                        spawn(async move {
                            match generate_ai_proxy_key().await {
                                Ok((raw_key, key_hash)) => {
                                    set_at_path(&mut form_values, &fp,
                                        serde_json::Value::String(key_hash));
                                    sync();
                                    generated_key.set(Some(raw_key));
                                }
                                Err(e) => {
                                    tracing::error!("key generation failed: {e}");
                                }
                            }
                        });
                    },
                    {t!("config-editor-generate")}
                }
            }
            if let Some(raw_key) = generated_key.read().as_ref() {
                div { class: "mt-2 alert alert-warn",
                    p { class: "text-xs font-semibold mb-1",
                        {t!("config-editor-key-warning")}
                    }
                    code { class: "block text-sm font-mono bg-surface p-2 rounded border border-line select-all break-all text-fg-strong",
                        "{raw_key}"
                    }
                    button { r#type: "button",
                        class: "mt-2 text-xs text-fg-muted hover:text-fg-strong underline",
                        onclick: move |_| {
                            generated_key.set(None);
                        },
                        {t!("config-editor-dismiss")}
                    }
                }
            }
        }
    }
}

/// Dedicated component for secret fields so we can use hooks (convert-to-secret state).
#[component]
fn SecretField(
    cluster_id: String,
    field_name: String,
    field_path: Vec<String>,
    mut form_values: Signal<serde_json::Value>,
    mut json_text: Signal<String>,
) -> Element {
    let mut converting = use_signal(|| false);
    let mut convert_error = use_signal(|| None::<String>);

    let val_str = get_at_path(&form_values.read(), &field_path)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    let is_already_ref = val_str.starts_with("secret:") || val_str.starts_with("env:");
    let has_value = !val_str.is_empty();

    let fp = field_path.clone();
    let fp2 = field_path.clone();
    let fp_convert = field_path.clone();
    let cid = cluster_id.clone();
    // Derive a secret name from the field path (e.g. "cloud.0.api_key" → "cloud_0_api_key")
    let derived_name = field_path
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("_");

    let mut sync = move || {
        let json = form_values.read().clone();
        json_text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
    };

    rsx! {
        input {
            r#type: "password",
            class: "border border-line rounded px-2 dark:bg-surface-2 dark:text-fg py-1 text-sm w-full",
            placeholder: t!("config-editor-secret-placeholder"),
            value: val_str,
            oninput: move |evt| {
                let v = evt.value();
                if v.is_empty() {
                    remove_at_path(&mut form_values, &fp);
                } else {
                    set_at_path(&mut form_values, &fp,
                        serde_json::Value::String(v));
                }
                sync();
            },
        }
        div { class: "flex items-center gap-2 mt-0.5",
            p { class: "text-xs text-fg-faint flex-1",
                {t!("config-editor-secret-hint")}
            }
            if has_value && !is_already_ref {
                button { r#type: "button",
                    class: "text-xs px-2 py-0.5 border border-warn text-warn-strong rounded hover:bg-warn-soft whitespace-nowrap",
                    disabled: *converting.read(),
                    onclick: move |evt| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        let cid = cid.clone();
                        let name = derived_name.clone();
                        let fp_inner = fp2.clone();
                        let value = get_at_path(&form_values.read(), &fp_convert)
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default();
                        converting.set(true);
                        convert_error.set(None);
                        spawn(async move {
                            match convert_to_secret(cid, name, value).await {
                                Ok(reference) => {
                                    set_at_path(&mut form_values, &fp_inner,
                                        serde_json::Value::String(reference));
                                    let json = form_values.read().clone();
                                    json_text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
                                }
                                Err(e) => {
                                    convert_error.set(Some(e.to_string()));
                                }
                            }
                            converting.set(false);
                        });
                    },
                    if *converting.read() { {t!("config-editor-converting")} } else { {t!("config-editor-convert-to-secret")} }
                }
            }
        }
        if let Some(err) = &*convert_error.read() {
            p { class: "text-xs text-danger mt-0.5", "{err}" }
        }
    }
}

/// Maximum characters shown in a placeholder before we add `…`.
/// Wide enough for typical schema descriptions, narrow enough that the
/// hint doesn't get clipped silently by the input width on desktop.
const PLACEHOLDER_MAX_CHARS: usize = 80;

/// Build the placeholder shown when a field has no schema `default`.
/// Falls back to "Enter {field}" when the schema doesn't carry a
/// description either. Long descriptions are truncated at the nearest
/// word boundary with a trailing ellipsis — never mid-word, never at a
/// stray abbreviation period (the previous "split on first `.`"
/// heuristic chopped `(e.g. "/ip4/…")` after the `e`).
fn placeholder_fallback(description: &str, field_label: &str) -> String {
    if description.is_empty() {
        return format!("Enter {field_label}");
    }
    let trimmed = description.trim();
    // Collapse newlines + tabs to a single space so multi-line schema
    // doc-comments stay legible inside a single-line input.
    let one_line: String = trimmed
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    truncate_with_ellipsis(&one_line, PLACEHOLDER_MAX_CHARS)
}

/// Truncate `s` to at most `max_chars` Unicode characters, breaking on
/// the last word boundary at or before the cut. Trailing punctuation
/// is stripped before the ellipsis so we don't end up with `..,…` or
/// `(e.g…`.
fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_string();
    }
    // Walk back from the cut to the last whitespace.
    let mut cut = max_chars;
    while cut > 0 && !chars[cut - 1].is_whitespace() && !chars[cut].is_whitespace() {
        cut -= 1;
    }
    if cut == 0 {
        // Single very long token — fall back to a hard cut.
        cut = max_chars;
    }
    let head: String = chars[..cut].iter().collect();
    let pruned = head.trim_end_matches([' ', '\t', '\n', '.', ',', ';', ':', '(', '/']);
    format!("{pruned}…")
}

#[cfg(test)]
mod placeholder_tests {
    use super::*;

    #[test]
    fn empty_description_falls_back_to_field_label() {
        assert_eq!(placeholder_fallback("", "host"), "Enter host");
    }

    #[test]
    fn short_description_passes_through() {
        let s = "Display name for this agent";
        assert_eq!(placeholder_fallback(s, "agent_name"), s);
    }

    #[test]
    fn description_with_inline_abbreviation_is_not_chopped_at_period() {
        // The bug report: "(e." was the cut site under the old
        // first-sentence heuristic.
        let desc = "Relay node libp2p multiaddress for circuit relay";
        assert_eq!(placeholder_fallback(desc, "relay_multiaddr"), desc);
    }

    #[test]
    fn long_description_truncates_at_word_boundary_with_ellipsis() {
        let desc = "Relay node libp2p multiaddress for circuit relay \
                    (e.g. \"/dns4/relay.example.com/tcp/4001/wss\")";
        let out = placeholder_fallback(desc, "relay_multiaddr");
        assert!(out.ends_with('…'), "expected ellipsis, got: {out}");
        assert!(
            !out.contains("(e…") && !out.contains(" e…"),
            "should not cut at the abbreviation period: {out}"
        );
        // Hard cap respected.
        assert!(out.chars().count() <= PLACEHOLDER_MAX_CHARS + 1);
    }

    #[test]
    fn newlines_collapse_to_single_spaces() {
        let desc = "Line one\n\nLine two";
        assert_eq!(placeholder_fallback(desc, "x"), "Line one Line two");
    }

    #[test]
    fn very_long_single_token_falls_back_to_hard_cut() {
        let desc = "a".repeat(200);
        let out = placeholder_fallback(&desc, "x");
        assert!(out.ends_with('…'));
        assert_eq!(out.chars().count(), PLACEHOLDER_MAX_CHARS + 1);
    }
}

fn resolve_ref(schema: &serde_json::Value, defs: &serde_json::Value) -> serde_json::Value {
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

/// Remove a value at a nested JSON path. Numeric segments index into arrays.
pub(super) fn remove_at_path(form_values: &mut Signal<serde_json::Value>, path: &[String]) {
    let mut val = form_values.write();
    let mut current = &mut *val;
    for key in &path[..path.len() - 1] {
        if let Ok(idx) = key.parse::<usize>() {
            if let serde_json::Value::Array(arr) = current {
                match arr.get_mut(idx) {
                    Some(next) => {
                        current = next;
                        continue;
                    }
                    None => return,
                }
            }
        }
        match current.get_mut(key.as_str()) {
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

/// Walk up the parent path of `path`, pruning any object that became
/// empty *and* was also absent in the saved baseline. Stops at the
/// first non-empty / baseline-present parent so intentional empty
/// objects in the saved JSON (e.g. `global: {}`, `cloud: []`) are
/// preserved. Arrays are deliberately left alone — an empty `cloud[0]`
/// slot is a real entry, not noise.
///
/// Used by both the reset arrow (after reverting a field) and the
/// regular oninput-empty handler (after the user types into a field
/// and backspaces it clean) so neither leaves orphan parent objects
/// that would keep the editor falsely dirty.
pub(super) fn cleanup_empty_parents(
    form_values: &mut Signal<serde_json::Value>,
    baseline: &serde_json::Value,
    path: &[String],
) {
    let mut prefix: Vec<String> = path.to_vec();
    prefix.pop();
    while !prefix.is_empty() {
        let cur_empty_object = get_at_path(&form_values.read(), &prefix)
            .as_ref()
            .and_then(|v| v.as_object())
            .is_some_and(|o| o.is_empty());
        let baseline_absent = get_at_path(baseline, &prefix).is_none();
        if cur_empty_object && baseline_absent {
            remove_at_path(form_values, &prefix);
            prefix.pop();
        } else {
            break;
        }
    }
}

/// Restore one field to its saved-baseline value, undoing any unsaved
/// edits. If the saved baseline doesn't contain the field, the field
/// is removed and any newly-empty parent objects absent in baseline
/// are pruned via `cleanup_empty_parents`. Without the cleanup, a
/// single `healer.auto_approve` edit followed by reset would leave
/// `healer: {}` in the form state — which the saved JSON doesn't
/// have, so the editor would still be dirty.
pub(super) fn revert_field_to_saved(
    form_values: &mut Signal<serde_json::Value>,
    baseline: &serde_json::Value,
    path: &[String],
) {
    if path.is_empty() {
        return;
    }
    match get_at_path(baseline, path) {
        Some(v) => set_at_path(form_values, path, v),
        None => {
            remove_at_path(form_values, path);
            cleanup_empty_parents(form_values, baseline, path);
        }
    }
}

/// Render a single entry inside an object array (e.g. one cloud
/// provider entry, or one ai_proxy.keys entry). Iterates the items
/// schema's properties and routes each child field through the same
/// `SectionFieldRow` used everywhere else, so array-entry fields share
/// the layout, `EditorBaseline` baseline, and reactivity rules with
/// top-level / subsection fields. Replaces the bespoke
/// `render_section_fields` path that compared against schema defaults
/// instead of the saved snapshot.
#[allow(clippy::too_many_arguments)]
fn render_object_array_entry(
    items_schema: &serde_json::Value,
    defs: &serde_json::Value,
    entry_path: Vec<String>,
    form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    cluster_id: String,
    _sync_to_json: impl Fn() + Clone + 'static,
) -> Element {
    let properties = items_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();
    let nested_section = entry_path.join(".");
    let defs_clone = defs.clone();
    rsx! {
        for (cname, cschema) in properties.into_iter() {
            {
                let key = format!("{nested_section}.{cname}");
                rsx! {
                    SectionFieldRow {
                        key: "{key}",
                        section_name: nested_section.clone(),
                        field_name: cname,
                        field_schema: cschema,
                        defs: defs_clone.clone(),
                        form_values,
                        json_text,
                        extra_config_open,
                        cluster_id: cluster_id.clone(),
                    }
                }
            }
        }
    }
}

/// Build a default JSON object from a schema (one level deep, for "add entry").
fn build_default_object(schema: &serde_json::Value, defs: &serde_json::Value) -> serde_json::Value {
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
                    "boolean" => {
                        obj.insert(key.clone(), serde_json::Value::Bool(true));
                    }
                    "string" => {}
                    _ => {}
                }
            }
        }
    }
    serde_json::Value::Object(obj)
}
