use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Settings {
    auto_trigger: bool,
    auto_trigger_key: String,
    auto_approve: bool,
    fix_model_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelOption {
    key: String,
    name: String,
    provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsData {
    settings: Settings,
    models: Vec<ModelOption>,
}

#[server]
async fn load_settings(cluster_id: String) -> Result<SettingsData, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid cluster id"))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        auto_trigger: Option<bool>,
        auto_trigger_provider: Option<String>,
        auto_trigger_model: Option<String>,
        auto_approve: Option<bool>,
        fix_provider: Option<String>,
        fix_model: Option<String>,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT auto_trigger, auto_trigger_provider, auto_trigger_model, \
                auto_approve, fix_provider, fix_model \
         FROM healer_cluster_settings WHERE cluster_id = $1",
    )
    .bind(cid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let settings = row
        .map(|r| Settings {
            auto_trigger: r.auto_trigger.unwrap_or(false),
            auto_trigger_key: match (r.auto_trigger_provider, r.auto_trigger_model) {
                (Some(p), Some(m)) => format!("{p}:{m}"),
                _ => "none".to_string(),
            },
            auto_approve: r.auto_approve.unwrap_or(false),
            fix_model_key: match (r.fix_provider, r.fix_model) {
                (Some(p), Some(m)) => format!("{p}:{m}"),
                _ => "none".to_string(),
            },
        })
        .unwrap_or_default();

    let healer_cfg = &crate::config::config().healer;
    let model_entries = if healer_cfg.models.is_empty() {
        crate::config::default_healer_models()
    } else {
        healer_cfg.models.clone()
    };
    let models = model_entries
        .into_iter()
        .map(|e| ModelOption {
            key: format!("{}:{}", e.provider, e.model),
            name: e.display_name(),
            provider: e.provider,
        })
        .collect();

    Ok(SettingsData { settings, models })
}

#[server]
async fn save_settings(
    cluster_id: String,
    auto_trigger: bool,
    auto_trigger_provider: Option<String>,
    auto_trigger_model: Option<String>,
    auto_approve: bool,
    fix_provider: Option<String>,
    fix_model: Option<String>,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid cluster id"))?;
    user.require_cluster_write(&pool, cid).await?;

    sqlx::query(
        "INSERT INTO healer_cluster_settings \
            (cluster_id, auto_trigger, auto_trigger_provider, auto_trigger_model, \
             auto_approve, fix_provider, fix_model, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, now()) \
         ON CONFLICT (cluster_id) DO UPDATE SET \
            auto_trigger = EXCLUDED.auto_trigger, \
            auto_trigger_provider = EXCLUDED.auto_trigger_provider, \
            auto_trigger_model = EXCLUDED.auto_trigger_model, \
            auto_approve = EXCLUDED.auto_approve, \
            fix_provider = EXCLUDED.fix_provider, \
            fix_model = EXCLUDED.fix_model, \
            updated_at = now()",
    )
    .bind(cid)
    .bind(auto_trigger)
    .bind(&auto_trigger_provider)
    .bind(&auto_trigger_model)
    .bind(auto_approve)
    .bind(&fix_provider)
    .bind(&fix_model)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

fn parse_key(key: &str) -> (Option<String>, Option<String>) {
    if key == "none" || key.is_empty() {
        (None, None)
    } else if let Some((p, m)) = key.split_once(':') {
        (Some(p.to_string()), Some(m.to_string()))
    } else {
        (None, None)
    }
}

#[component]
pub fn ClusterHealerSettings(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut data_future = use_server_future(move || {
        let cid = cid.clone();
        async move { load_settings(cid).await }
    })?;

    let Some(Ok(data)) = &*data_future.read() else {
        return rsx! { p { class: "text-sm text-gray-500", "Loading..." } };
    };

    let models = data.models.clone();
    let mut auto_trigger = use_signal(move || data.settings.auto_trigger);
    let mut auto_trigger_key = use_signal(move || data.settings.auto_trigger_key.clone());
    let mut auto_approve = use_signal(move || data.settings.auto_approve);
    let mut fix_model_key = use_signal(move || data.settings.fix_model_key.clone());
    let mut saving = use_signal(|| false);

    let ollama: Vec<_> = models.iter().filter(|m| m.provider == "ollama").cloned().collect();
    let anthropic: Vec<_> = models.iter().filter(|m| m.provider == "anthropic").cloned().collect();
    let openrouter: Vec<_> = models.iter().filter(|m| m.provider == "openrouter").cloned().collect();

    rsx! {
        div { class: "space-y-3",
            // Auto-trigger
            div { class: "flex items-center gap-2",
                input {
                    r#type: "checkbox",
                    disabled: read_only,
                    checked: *auto_trigger.read(),
                    onchange: move |e| auto_trigger.set(e.checked()),
                }
                span { class: "text-sm", "Auto-trigger" }
            }
            // Auto-trigger model
            div {
                label { class: "block text-xs text-gray-500 dark:text-gray-400 mb-1", "Auto-trigger model" }
                select {
                    class: "w-full px-2 py-1 text-sm border rounded dark:bg-gray-700 dark:border-gray-600 dark:text-gray-200",
                    disabled: read_only,
                    value: "{auto_trigger_key}",
                    onchange: move |e| auto_trigger_key.set(e.value()),
                    option { value: "none", "Server default" }
                    {model_optgroups(&ollama, &anthropic, &openrouter)}
                }
            }
            // Auto-approve
            div { class: "flex items-center gap-2",
                input {
                    r#type: "checkbox",
                    disabled: read_only,
                    checked: *auto_approve.read(),
                    onchange: move |e| auto_approve.set(e.checked()),
                }
                span { class: "text-sm", "Auto-approve remediation" }
            }
            // Fix model
            div {
                label { class: "block text-xs text-gray-500 dark:text-gray-400 mb-1", "Fix model (remediation)" }
                select {
                    class: "w-full px-2 py-1 text-sm border rounded dark:bg-gray-700 dark:border-gray-600 dark:text-gray-200",
                    disabled: read_only,
                    value: "{fix_model_key}",
                    onchange: move |e| fix_model_key.set(e.value()),
                    option { value: "none", "Same as diagnosis" }
                    {model_optgroups(&ollama, &anthropic, &openrouter)}
                }
            }
            if !read_only {
                button {
                    class: "px-3 py-1 text-sm font-medium bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50",
                    disabled: *saving.read(),
                    onclick: {
                        let cluster_id = cluster_id.clone();
                        move |_| {
                            let cid = cluster_id.clone();
                            let at = *auto_trigger.read();
                            let aa = *auto_approve.read();
                            let at_key = auto_trigger_key.read().clone();
                            let fix_key = fix_model_key.read().clone();
                            let (atp, atm) = parse_key(&at_key);
                            let (fp, fm) = parse_key(&fix_key);
                            saving.set(true);
                            async move {
                                let _ = save_settings(cid, at, atp, atm, aa, fp, fm).await;
                                saving.set(false);
                                data_future.restart();
                            }
                        }
                    },
                    if *saving.read() { "Saving..." } else { "Save" }
                }
            }
        }
    }
}

fn model_optgroups(
    ollama: &[ModelOption],
    anthropic: &[ModelOption],
    openrouter: &[ModelOption],
) -> Element {
    rsx! {
        if !ollama.is_empty() {
            optgroup { label: "Ollama (local)",
                for m in ollama.iter() {
                    option { value: "{m.key}", "{m.name}" }
                }
            }
        }
        if !anthropic.is_empty() {
            optgroup { label: "Anthropic",
                for m in anthropic.iter() {
                    option { value: "{m.key}", "{m.name}" }
                }
            }
        }
        if !openrouter.is_empty() {
            optgroup { label: "OpenRouter",
                for m in openrouter.iter() {
                    option { value: "{m.key}", "{m.name}" }
                }
            }
        }
    }
}
