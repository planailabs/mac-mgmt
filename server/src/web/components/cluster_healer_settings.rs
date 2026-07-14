use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::clusters::{
    HealerModelOption, HealerSettingsInput, HealerSettingsSetInput, load_settings, save_settings,
};
use crate::web::components::ui::{Button, ButtonSize, HelpText};

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
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|_| ServerFnError::new("invalid cluster id"))?;
            load_settings(HealerSettingsInput { id }).await
        }
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

    let ollama: Vec<_> = models
        .iter()
        .filter(|m| m.provider == "ollama")
        .cloned()
        .collect();
    let anthropic: Vec<_> = models
        .iter()
        .filter(|m| m.provider == "anthropic")
        .cloned()
        .collect();
    let openrouter: Vec<_> = models
        .iter()
        .filter(|m| m.provider == "openrouter")
        .cloned()
        .collect();
    // Everything else is a named OpenAI-compatible source.
    let other: Vec<_> = models
        .iter()
        .filter(|m| !matches!(m.provider.as_str(), "ollama" | "anthropic" | "openrouter"))
        .cloned()
        .collect();

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
                            {model_optgroups(&ollama, &anthropic, &openrouter, &other)}
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
                            {model_optgroups(&ollama, &anthropic, &openrouter, &other)}
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
                                if let Ok(id) = cid.parse::<uuid::Uuid>() {
                                    let _ = save_settings(HealerSettingsSetInput {
                                        id,
                                        enabled: en,
                                        auto_trigger: at,
                                        auto_trigger_provider: atp,
                                        auto_trigger_model: atm,
                                        auto_approve: aa,
                                        fix_provider: fp,
                                        fix_model: fm,
                                    })
                                    .await;
                                }
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
    ollama: &[HealerModelOption],
    anthropic: &[HealerModelOption],
    openrouter: &[HealerModelOption],
    other: &[HealerModelOption],
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
        if !other.is_empty() {
            optgroup { label: "OpenAI-compatible",
                for m in other.iter() {
                    option { value: "{m.key}", "{m.name}" }
                }
            }
        }
    }
}
