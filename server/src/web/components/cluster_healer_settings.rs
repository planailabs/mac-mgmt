use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::components::ui::{Button, ButtonSize, HelpText};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Settings {
    enabled: bool,
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
        enabled: bool,
        auto_trigger: Option<bool>,
        auto_trigger_provider: Option<String>,
        auto_trigger_model: Option<String>,
        auto_approve: Option<bool>,
        fix_provider: Option<String>,
        fix_model: Option<String>,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT enabled, auto_trigger, auto_trigger_provider, auto_trigger_model, \
                auto_approve, fix_provider, fix_model \
         FROM healer_cluster_settings WHERE cluster_id = $1",
    )
    .bind(cid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let settings = row
        .map(|r| Settings {
            enabled: r.enabled,
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
    enabled: bool,
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
            (cluster_id, enabled, auto_trigger, auto_trigger_provider, auto_trigger_model, \
             auto_approve, fix_provider, fix_model, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now()) \
         ON CONFLICT (cluster_id) DO UPDATE SET \
            enabled = EXCLUDED.enabled, \
            auto_trigger = EXCLUDED.auto_trigger, \
            auto_trigger_provider = EXCLUDED.auto_trigger_provider, \
            auto_trigger_model = EXCLUDED.auto_trigger_model, \
            auto_approve = EXCLUDED.auto_approve, \
            fix_provider = EXCLUDED.fix_provider, \
            fix_model = EXCLUDED.fix_model, \
            updated_at = now()",
    )
    .bind(cid)
    .bind(enabled)
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
        return rsx! { HelpText { {t!("loading")} } };
    };

    let models = data.models.clone();
    let mut enabled = use_signal(move || data.settings.enabled);
    let mut auto_trigger = use_signal(move || data.settings.auto_trigger);
    let mut auto_trigger_key = use_signal(move || data.settings.auto_trigger_key.clone());
    let mut auto_approve = use_signal(move || data.settings.auto_approve);
    let mut fix_model_key = use_signal(move || data.settings.fix_model_key.clone());
    let mut saving = use_signal(|| false);

    let ollama: Vec<_> = models.iter().filter(|m| m.provider == "ollama").cloned().collect();
    let anthropic: Vec<_> = models.iter().filter(|m| m.provider == "anthropic").cloned().collect();
    let openrouter: Vec<_> = models.iter().filter(|m| m.provider == "openrouter").cloned().collect();

    let is_enabled = *enabled.read();
    let fields_disabled = read_only || !is_enabled;

    rsx! {
        div { class: "space-y-3",
            // Override enabled toggle
            div { class: "flex items-center gap-2",
                input {
                    r#type: "checkbox",
                    disabled: read_only,
                    checked: is_enabled,
                    class: "rounded border-line",
                    onchange: move |e| enabled.set(e.checked()),
                }
                span { class: "text-sm font-medium text-fg-strong", {t!("healer-settings-override")} }
                if !is_enabled {
                    span { class: "text-xs text-fg-faint", {t!("healer-settings-using-defaults")} }
                }
            }

            div { class: if is_enabled { "" } else { "opacity-50 pointer-events-none" },
                div { class: "space-y-3",
                    // Auto-trigger
                    div { class: "flex items-center gap-2",
                        input {
                            r#type: "checkbox",
                            disabled: fields_disabled,
                            checked: *auto_trigger.read(),
                            class: "rounded border-line",
                            onchange: move |e| auto_trigger.set(e.checked()),
                        }
                        span { class: "text-sm text-fg", {t!("healer-settings-auto-trigger")} }
                    }
                    // Auto-trigger model
                    div {
                        label { class: "block help-xs mb-1", {t!("healer-settings-auto-trigger-model")} }
                        select { class: "input input-sm",
                            disabled: fields_disabled,
                            value: "{auto_trigger_key}",
                            onchange: move |e| auto_trigger_key.set(e.value()),
                            option { value: "none", {t!("healer-settings-server-default")} }
                            {model_optgroups(&ollama, &anthropic, &openrouter)}
                        }
                    }
                    // Auto-approve
                    div { class: "flex items-center gap-2",
                        input {
                            r#type: "checkbox",
                            disabled: fields_disabled,
                            checked: *auto_approve.read(),
                            class: "rounded border-line",
                            onchange: move |e| auto_approve.set(e.checked()),
                        }
                        span { class: "text-sm text-fg", {t!("healer-settings-auto-approve")} }
                    }
                    // Fix model
                    div {
                        label { class: "block help-xs mb-1", {t!("healer-settings-fix-model")} }
                        select { class: "input input-sm",
                            disabled: fields_disabled,
                            value: "{fix_model_key}",
                            onchange: move |e| fix_model_key.set(e.value()),
                            option { value: "none", {t!("healer-settings-same-as-diagnosis")} }
                            {model_optgroups(&ollama, &anthropic, &openrouter)}
                        }
                    }
                }
            }

            if !read_only {
                Button {
                    size: ButtonSize::Sm,
                    disabled: *saving.read(),
                    onclick: {
                        let cluster_id = cluster_id.clone();
                        move |_| {
                            let cid = cluster_id.clone();
                            let en = *enabled.read();
                            let at = *auto_trigger.read();
                            let aa = *auto_approve.read();
                            let at_key = auto_trigger_key.read().clone();
                            let fix_key = fix_model_key.read().clone();
                            let (atp, atm) = parse_key(&at_key);
                            let (fp, fm) = parse_key(&fix_key);
                            saving.set(true);
                            async move {
                                let _ = save_settings(cid, en, at, atp, atm, aa, fp, fm).await;
                                saving.set(false);
                                data_future.restart();
                            }
                        }
                    },
                    if *saving.read() { {t!("healer-settings-saving")} } else { {t!("save")} }
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
            optgroup { label: t!("healer-settings-ollama"),
                for m in ollama.iter() {
                    option { value: "{m.key}", "{m.name}" }
                }
            }
        }
        if !anthropic.is_empty() {
            optgroup { label: t!("healer-settings-anthropic"),
                for m in anthropic.iter() {
                    option { value: "{m.key}", "{m.name}" }
                }
            }
        }
        if !openrouter.is_empty() {
            optgroup { label: t!("healer-settings-openrouter"),
                for m in openrouter.iter() {
                    option { value: "{m.key}", "{m.name}" }
                }
            }
        }
    }
}
