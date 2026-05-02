use dioxus::prelude::*;
use dioxus_i18n::t;

use super::extra_config_modal::{ExtraConfigField, ExtraConfigModalHost};
use crate::models::ClusterConfig;
#[cfg(feature = "server")]
use crate::web::user::current_user;

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

/// Last-saved baseline for the cluster config — read by the floating
/// SaveBar to compute "unsaved changes" without re-querying the server
/// every keystroke.
#[derive(Clone, Default)]
pub struct SavedConfigBaseline(pub Signal<String>);

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

    if !*initialized.read() {
        if let Some(Ok(Some(cfg))) = &*config.read() {
            let pretty = serde_json::to_string_pretty(&cfg.config_json).unwrap_or_default();
            editor_text.set(pretty.clone());
            saved_text.set(pretty);
            initialized.set(true);
        }
    }

    let cid_save = cluster_id.clone();
    let do_save = move || {
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
    };

    let do_discard = move |_| {
        let baseline = saved_text.read().clone();
        editor_text.set(baseline);
    };

    // Diff-aware dirty flag — fast string compare, no JSON re-parse on
    // every keystroke. `saved_text` only mutates after a successful save
    // or a discard, so this stays O(1) on the typical input path.
    let dirty = *editor_text.read() != *saved_text.read() && *initialized.read();

    let last_saved_at: Option<String> = match &*config.read() {
        Some(Ok(Some(cfg))) => Some(cfg.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
        _ => None,
    };

    rsx! {
        if let Some(err) = &*error.read() {
            p { class: "text-danger text-sm mb-2", "{err}" }
        }

        // Sticky filter / mode strip — under the page header. Mirrors
        // the design's "All / Enabled / Modified / Errors" pill row.
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

        div { class: "pb-32",
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
        // viewport with brand-orange ring, mirroring the Cmd+S
        // affordance described in the design.
        if !read_only && dirty {
            SaveBar {
                last_saved: last_saved_at,
                on_save: do_save,
                on_discard: do_discard,
            }
        }
    }
}

#[component]
fn FilterChips(filter: Signal<SectionFilter>) -> Element {
    let cur = *filter.read();
    let chip = |opt: SectionFilter, label_key: &'static str| {
        let active = cur == opt;
        let cls = if active {
            "config-chip config-chip-active"
        } else {
            "config-chip"
        };
        rsx! {
            button {
                class: "{cls}",
                onclick: move |_| filter.set(opt),
                {t!(label_key)}
            }
        }
    };
    rsx! {
        div { class: "config-chips",
            {chip(SectionFilter::All, "config-filter-all")}
            {chip(SectionFilter::Enabled, "config-filter-enabled")}
            {chip(SectionFilter::Modified, "config-filter-modified")}
            {chip(SectionFilter::Errors, "config-filter-errors")}
        }
    }
}

#[component]
fn SaveBar(
    last_saved: Option<String>,
    on_save: EventHandler<()>,
    on_discard: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "config-save-bar",
            span { class: "dot dot-accent" }
            div { class: "min-w-0",
                div { class: "text-sm font-semibold text-fg-strong", {t!("config-save-unsaved")} }
                if let Some(t) = last_saved {
                    div { class: "text-[11px] text-fg-faint mt-0.5",
                        {t!("config-editor-last-saved", time: t)}
                    }
                }
            }
            div { class: "h-7 w-px bg-line mx-1" }
            button {
                class: "btn btn-md btn-secondary",
                r#type: "button",
                onclick: on_discard,
                {t!("config-save-discard")}
            }
            button {
                class: "btn btn-md btn-primary",
                r#type: "button",
                onclick: move |_| on_save.call(()),
                {t!("config-editor-save")}
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
    let defs_for_filter = defs.clone();
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
                let props = s
                    .resolved
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .cloned()
                    .unwrap_or_default();
                props.iter().any(|(fname, fschema)| {
                    if fname == "enabled" {
                        return false;
                    }
                    let resolved = resolve_ref(fschema, &defs_for_filter);
                    let schema_default = resolved.get("default");
                    let current =
                        get_at_path(&form_snapshot, &[s.name.clone(), fname.clone()]);
                    match (current, schema_default) {
                        (Some(cur), Some(def)) => &cur != def,
                        (Some(_), None) => true,
                        _ => false,
                    }
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
        div { class: "flex gap-6",
            // Main content
            div { class: "flex-1 min-w-0 space-y-8 mb-4",
                {categories.into_iter().filter_map(|(category_id, sections)| {
                    let visible: Vec<SectionMeta> = sections.into_iter().filter(|s| section_passes_filter(s)).collect();
                    if visible.is_empty() {
                        return None;
                    }
                    Some((category_id, visible))
                }).enumerate().map(|(cat_idx, (category_id, sections))| {
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
                                div { class: "flex items-baseline gap-3",
                                    span { class: "kicker font-mono",
                                        "§ {cat_idx + 1:02}"
                                    }
                                    h3 { class: "text-[15px] font-semibold text-fg-strong tracking-tight",
                                        {t!(&cat_i18n_key)}
                                    }
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
                                    rsx! {
                                        a {
                                            key: "{name}",
                                            href: "#sec-{name}",
                                            class: "flex items-center gap-2 px-2 py-1 rounded-md text-xs font-mono text-fg-muted hover:text-fg-strong hover:bg-surface-2 transition-colors",
                                            span { class: "{dot_cls}" }
                                            span { class: "truncate", "{name_display}" }
                                        }
                                    }
                                })}
                            }
                        }
                    })}
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

    // Toggle handler that also writes back to sessionStorage so the
    // change survives a navigation away and back.
    let cid_persist = cluster_id.clone();
    let sn_persist = section_name.clone();
    let mut toggle_expanded = move || {
        let new_val = !*expanded.read();
        expanded.set(new_val);
        let cid = cid_persist.clone();
        let sn = sn_persist.clone();
        let new_v = if new_val { "true" } else { "false" };
        let script = format!(
            "try {{ \
              var k = 'cluster-cfg.' + {cid:?} + '.expanded'; \
              var v; try {{ v = JSON.parse(sessionStorage.getItem(k) || '[]'); }} catch(e) {{ v = []; }} \
              var idx = v.indexOf({sn:?}); \
              if ({nv} && idx === -1) v.push({sn:?}); \
              if (!{nv} && idx !== -1) v.splice(idx, 1); \
              sessionStorage.setItem(k, JSON.stringify(v)); \
            }} catch(e) {{}}",
            cid = cid,
            sn = sn,
            nv = new_v
        );
        document::eval(&script);
    };

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

    // Count modified fields (non-default)
    let mod_count = properties.iter().filter(|(fname, fschema)| {
        if fname.as_str() == "enabled" { return false; }
        let resolved = resolve_ref(fschema, &defs);
        let schema_default = resolved.get("default");
        let current = get_at_path(&form_values.read(), &[section_name.clone(), fname.to_string()]);
        match (current, schema_default) {
            (Some(cur), Some(def)) => &cur != def,
            (Some(_), None) => true,
            _ => false,
        }
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

    let show_body = *expanded.read() && (always_on || is_enabled);
    let section_name_toggle = section_name.clone();
    let section_name_display = section_name.clone();
    let section_anchor = format!("sec-{section_name}");

    rsx! {
        div { class: "card overflow-hidden scroll-mt-20", id: "{section_anchor}",
            // Header
            div {
                class: "flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-surface-2 transition-colors",
                onclick: move |_| toggle_expanded(),
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
    let show_body = *expanded.read() && is_enabled;
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

/// Render the "+ Add entry" button for array sections.
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
    rsx! {
        button {
            r#type: "button",
            class: "btn btn-sm btn-ghost w-full border border-dashed border-line text-fg-muted",
            onclick: move |_| {
                let mut arr = get_at_path(&form_values.read(), &path)
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default();
                let new_obj = build_default_object(&items_schema, &defs);
                arr.push(new_obj);
                set_at_path(&mut form_values, &path, serde_json::Value::Array(arr));
                json_text.set(serde_json::to_string_pretty(&*form_values.read()).unwrap_or_default());
            },
            {t!("config-editor-add-entry")}
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

    let is_non_default = current_value.as_ref().is_some_and(|v| {
        schema_default.as_ref().map_or(true, |d| v != d)
    });

    let sync = move || {
        let json = form_values.read().clone();
        let mut text = json_text;
        text.set(serde_json::to_string_pretty(&json).unwrap_or_default());
    };

    // Special-case: extra_config under openclaw
    if field_name == "extra_config" && section_name.contains("openclaw") {
        return rsx! {
            ExtraConfigField {
                form_values,
                open: extra_config_open,
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
        // Render as a nested subsection
        return rsx! {
            div { class: "border-t border-line px-4 py-3",
                div { class: "border-l-2 border-info pl-3",
                    label { class: "text-sm font-semibold text-info", "{field_name}" }
                    if !description.is_empty() {
                        p { class: "text-xs text-fg-muted mt-0.5", "{description}" }
                    }
                    {render_section_fields(
                        &resolved,
                        &defs,
                        field_path,
                        form_values,
                        json_text,
                        extra_config_open,
                        cluster_id,
                        sync,
                    )}
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
            div { class: "grid grid-cols-[200px_1fr_28px] gap-3 items-start",
                // Label column
                div { class: if field_type == "boolean" { "pt-0" } else { "pt-1.5" },
                    div { class: "flex items-center gap-1.5",
                        span { class: "text-xs font-medium font-mono text-fg-strong", "{field_name}" }
                        if is_non_default {
                            span { class: "w-1.5 h-1.5 rounded-full bg-brand", title: "modified" }
                        }
                    }
                    if !description.is_empty() {
                        p { class: "text-[11px] text-fg-faint mt-0.5 leading-tight", "{description}" }
                    }
                }
                // Input column
                div { class: "min-w-0",
                    {render_field_input(
                        &field_type,
                        &resolved,
                        &defs,
                        is_secret,
                        current_value.clone(),
                        fp.clone(),
                        fp2.clone(),
                        form_values,
                        json_text,
                        extra_config_open,
                        cluster_id.clone(),
                        sync.clone(),
                        field_name.clone(),
                    )}
                }
                // Reset column
                div { class: "flex justify-end pt-1",
                    if is_non_default {
                        button {
                            r#type: "button",
                            class: "text-fg-faint hover:text-danger text-xs",
                            title: t!("config-editor-reset-default"),
                            onclick: move |evt| {
                                evt.prevent_default();
                                evt.stop_propagation();
                                if let Some(ref def) = reset_default {
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
#[allow(clippy::too_many_arguments)]
fn render_field_input(
    field_type: &str,
    resolved: &serde_json::Value,
    defs: &serde_json::Value,
    is_secret: bool,
    current_value: Option<serde_json::Value>,
    fp: Vec<String>,
    fp2: Vec<String>,
    mut form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    cluster_id: String,
    sync: impl Fn() + Clone + 'static,
    field_name: String,
) -> Element {
    match field_type {
        "boolean" => {
            let checked = current_value
                .as_ref()
                .and_then(|v| v.as_bool())
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
            let sync_c = sync.clone();
            rsx! {
                input {
                    r#type: "number",
                    class: "input input-sm font-mono",
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
            let sync_c = sync.clone();
            if !enum_values.is_empty() {
                let selected_val = val_str.clone();
                rsx! {
                    div { class: "relative",
                        select {
                            class: "input input-sm font-mono appearance-none pr-8",
                            value: val_str,
                            onchange: move |evt| {
                                set_at_path(&mut form_values, &fp,
                                    serde_json::Value::String(evt.value()));
                                sync_c();
                            },
                            option { value: "", selected: selected_val.is_empty(), {t!("config-editor-select")} }
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
                rsx! {
                    input {
                        r#type: "text",
                        class: "input input-sm font-mono",
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

/// Render fields for one config section, including nested subsections.
fn render_section_fields(
    section_schema: &serde_json::Value,
    defs: &serde_json::Value,
    path: Vec<String>,
    mut form_values: Signal<serde_json::Value>,
    json_text: Signal<String>,
    extra_config_open: Signal<bool>,
    cluster_id: String,
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

            // Special-case: key_hash under ai_proxy.keys gets a "Generate" button.
            if field_name == "key_hash" && path.len() >= 2 && path.get(path.len() - 2).map(|s| s.as_str()) == Some("keys") {
                let mut field_path = path.clone();
                field_path.push(field_name.clone());
                let key = field_path.join(".");
                return rsx! {
                    KeyHashField {
                        key: "{key}",
                        field_path,
                        form_values,
                        json_text,
                    }
                };
            }

            let resolved = resolve_ref(&field_schema, defs);
            // Check x-secret extension (Secret type emits this in its JSON Schema)
            let is_secret = resolved.get("x-secret").and_then(|v| v.as_bool()).unwrap_or(false)
                || field_schema.get("x-secret").and_then(|v| v.as_bool()).unwrap_or(false);
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
                    div { class: "border-l-2 border-info pl-3 mt-2 mb-1",
                        key: "{key}",
                        label { class: "text-sm font-semibold text-info", "{field_name}" }
                        if !description.is_empty() {
                            p { class: "text-xs text-fg-muted", "{description}" }
                        }
                        {render_section_fields(
                            &resolved,
                            &defs_clone,
                            field_path,
                            form_values,
                            json_text,
                            extra_config_open,
                            cluster_id.clone(),
                            sync,
                        )}
                    }
                }
            } else {
                let fp = field_path.clone();
                let fp2 = field_path.clone();
                let schema_default = resolved.get("default").cloned();
                let is_non_default = current_value.as_ref().is_some_and(|v| {
                    schema_default.as_ref().map_or(true, |d| v != d)
                });
                let reset_path = field_path.clone();
                let reset_default = schema_default.clone();
                let sync_reset = sync_to_json.clone();
                rsx! {
                    div { class: "flex flex-col gap-0.5",
                        key: "{key}",
                        div { class: "flex items-center justify-between gap-2",
                            label { class: "text-sm font-medium text-fg-strong", "{field_name}" }
                            if is_non_default {
                                button {
                                    r#type: "button",
                                    class: "text-xs text-fg-muted hover:text-danger underline",
                                    onclick: move |evt| {
                                        evt.prevent_default();
                                        evt.stop_propagation();
                                        if let Some(ref def) = reset_default {
                                            set_at_path(&mut form_values, &reset_path, def.clone());
                                        } else {
                                            remove_at_path(&mut form_values, &reset_path);
                                        }
                                        sync_reset();
                                    },
                                    {t!("config-editor-reset-default")}
                                }
                            }
                        }
                        if !description.is_empty() {
                            p { class: "text-xs text-fg-muted", "{description}" }
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
                                        class: "border border-line rounded px-2 dark:bg-surface-2 dark:text-fg py-1 text-sm w-full",
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
                                                        div { class: "border border-line rounded p-2 relative",
                                                            key: "{idx}",
                                                            div { class: "flex justify-between items-center mb-1",
                                                                span { class: "text-xs font-semibold text-fg-muted", "#{idx}" }
                                                                button {
                                                                    class: "link-danger text-xs px-1",
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
                                                                cluster_id.clone(),
                                                                sync_remove.clone(),
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
                                                    // Add an empty object; defaults come from schema.
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
                                                    span { class: "flex-1 text-sm font-mono bg-surface-2 border border-line-soft rounded px-2 py-0.5 truncate",
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
                                                            class: "flex-1 border border-line rounded px-2 py-0.5 text-sm dark:bg-surface-2 dark:text-fg",
                                                            placeholder: t!("config-editor-add-item"),
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
                                                            class: "btn btn-xs btn-primary",
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
                                    let selected_val = val_str.clone();
                                    rsx! {
                                        select {
                                            class: "border border-line rounded px-2 dark:bg-surface-2 dark:text-fg py-1 text-sm w-full",
                                            value: val_str,
                                            onchange: move |evt| {
                                                set_at_path(&mut form_values, &fp,
                                                    serde_json::Value::String(evt.value()));
                                                sync_c();
                                            },
                                            option { value: "", selected: selected_val.is_empty(), {t!("config-editor-select")} }
                                            {enum_values.iter().map(|v| {
                                                let is_selected = *v == selected_val;
                                                let v = v.clone();
                                                rsx! { option { value: "{v}", selected: is_selected, "{v}" } }
                                            })}
                                        }
                                    }
                                } else if is_secret {
                                    let secret_key = field_path.join(".");
                                    rsx! {
                                        SecretField {
                                            key: "{secret_key}",
                                            cluster_id: cluster_id.clone(),
                                            field_name: field_name.clone(),
                                            field_path,
                                            form_values,
                                            json_text,
                                        }
                                    }
                                } else {
                                    rsx! {
                                        input {
                                            r#type: "text",
                                            class: "border border-line rounded px-2 dark:bg-surface-2 dark:text-fg py-1 text-sm w-full",
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
    cluster_id: String,
    sync_to_json: impl Fn() + Clone + 'static,
) -> Element {
    render_section_fields(
        items_schema,
        defs,
        entry_path,
        form_values,
        json_text,
        extra_config_open,
        cluster_id,
        sync_to_json,
    )
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
