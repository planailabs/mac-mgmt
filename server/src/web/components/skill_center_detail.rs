use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::skill_centers::{
    CatalogSummaryInput, SkillCenterDeleteInput, SkillCenterGetInput, SkillCenterSyncInput,
    SkillCenterUpdateInput, delete_skill_center, get_catalog_summary, get_skill_center,
    sync_skill_center_now, update_skill_center,
};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonKind, ButtonSize, ButtonVariant, ErrorText, FormField, HelpText, PageHeader,
    SectionHeading,
};

#[component]
pub fn SkillCenterDetail(id: String) -> Element {
    use_topbar(t!("nav-skill-centers"), None);
    let id2 = id.clone();
    let id3 = id.clone();
    let mut center_future = use_server_future(move || {
        let id = id.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_skill_center(SkillCenterGetInput { id }).await
        }
    })?;

    let mut catalog_summary = use_server_future(move || {
        let id = id2.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_catalog_summary(CatalogSummaryInput { id }).await
        }
    })?;

    let mut syncing = use_signal(|| false);
    let mut sync_error = use_signal(|| None::<String>);

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_url = use_signal(String::new);
    let mut draft_token = use_signal(String::new);
    let mut draft_priority = use_signal(|| "0".to_string());
    let mut draft_enabled = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let nav = navigator();

    rsx! {
        div { class: "px-6 py-8 max-w-3xl mx-auto",
            match &*center_future.read() {
                Some(Ok(Some(center))) => {
                    let center_id = center.id.to_string();
                    let center_name = center.name.clone();
                    let center_url = center.url.clone();
                    let center_url_display = center.url.clone();
                    let center_priority = center.priority;
                    let center_enabled = center.enabled;

                    rsx! {
                        div { class: "flex items-center justify-between mb-6",
                            PageHeader { class: "mb-0", "{center_name}" }
                            div { class: "flex gap-2",
                                if !*editing.read() && !center_url_display.starts_with("builtin://") {
                                    Button {
                                        variant: ButtonVariant::Secondary,
                                        size: ButtonSize::Md,
                                        onclick: move |_| {
                                            draft_name.set(center_name.clone());
                                            draft_url.set(center_url.clone());
                                            draft_token.set(String::new());
                                            draft_priority.set(center_priority.to_string());
                                            draft_enabled.set(center_enabled);
                                            editing.set(true);
                                        },
                                        {t!("edit")}
                                    }
                                    Button {
                                        variant: ButtonVariant::Danger,
                                        size: ButtonSize::Md,
                                        onclick: {
                                            let id = center_id.clone();
                                            move |_| {
                                                let id = id.clone();
                                                spawn(async move {
                                                    let Ok(id) = id.parse::<uuid::Uuid>() else {
                                                        tracing::error!("delete failed: invalid UUID");
                                                        return;
                                                    };
                                                    if let Err(e) = delete_skill_center(SkillCenterDeleteInput { id }).await {
                                                        tracing::error!("delete failed: {e}");
                                                    } else {
                                                        nav.push(Route::SkillCenterList {});
                                                    }
                                                });
                                            }
                                        },
                                        {t!("delete")}
                                    }
                                }
                            }
                        }

                        if *editing.read() {
                            form {
                                onsubmit: {
                                    let id = center_id.clone();
                                    move |e: Event<FormData>| {
                                        e.prevent_default();
                                        let id = id.clone();
                                        let name = draft_name.read().clone();
                                        let url = draft_url.read().clone();
                                        let token = draft_token.read().clone();
                                        let priority: i32 = draft_priority.read().parse().unwrap_or(0);
                                        let enabled = *draft_enabled.read();
                                        spawn(async move {
                                            let Ok(id) = id.parse::<uuid::Uuid>() else {
                                                error.set(Some("invalid UUID".to_string()));
                                                return;
                                            };
                                            match update_skill_center(SkillCenterUpdateInput {
                                                id,
                                                name,
                                                url,
                                                federation_token: token,
                                                priority,
                                                enabled,
                                            }).await {
                                                Ok(_) => {
                                                    editing.set(false);
                                                    error.set(None);
                                                    center_future.restart();
                                                }
                                                Err(e) => error.set(Some(e.to_string())),
                                            }
                                        });
                                    }
                                },
                                if let Some(err) = &*error.read() {
                                    ErrorText { class: "mb-4", "{err}" }
                                }
                                FormField { label: t!("name"),
                                    input {
                                        class: "input",
                                        r#type: "text",
                                        value: "{draft_name}",
                                        oninput: move |e| draft_name.set(e.value()),
                                    }
                                }
                                FormField { label: t!("skill-center-detail-url"),
                                    input {
                                        class: "input",
                                        r#type: "text",
                                        value: "{draft_url}",
                                        oninput: move |e| draft_url.set(e.value()),
                                    }
                                }
                                FormField { label: t!("skill-center-detail-token-label"),
                                    input {
                                        class: "input",
                                        r#type: "password",
                                        value: "{draft_token}",
                                        oninput: move |e| draft_token.set(e.value()),
                                        placeholder: t!("skill-center-detail-token-hint"),
                                    }
                                }
                                FormField { label: t!("skill-center-detail-priority"),
                                    input {
                                        class: "input",
                                        r#type: "number",
                                        value: "{draft_priority}",
                                        oninput: move |e| draft_priority.set(e.value()),
                                    }
                                }
                                div { class: "flex items-center gap-2 mb-4",
                                    input {
                                        r#type: "checkbox",
                                        checked: "{draft_enabled}",
                                        oninput: move |e| draft_enabled.set(e.value() == "true"),
                                        class: "rounded border-line",
                                        id: "edit-enabled",
                                    }
                                    label { r#for: "edit-enabled", class: "text-sm text-fg",
                                        {t!("skill-center-detail-enabled")}
                                    }
                                }
                                div { class: "flex gap-2",
                                    Button { kind: ButtonKind::Submit, {t!("save")} }
                                    Button {
                                        variant: ButtonVariant::Secondary,
                                        onclick: move |_| { editing.set(false); error.set(None); },
                                        {t!("cancel")}
                                    }
                                }
                            }
                        } else {
                            dl { class: "grid grid-cols-2 gap-x-4 gap-y-2 text-sm mt-4",
                                dt { class: "font-medium text-fg-strong", {t!("skill-center-detail-url")} }
                                dd { class: "text-fg-muted", "{center_url_display}" }
                                dt { class: "font-medium text-fg-strong", {t!("skill-center-detail-priority")} }
                                dd { class: "text-fg-muted", "{center_priority}" }
                                dt { class: "font-medium text-fg-strong", {t!("skill-center-detail-enabled")} }
                                dd { class: "text-fg-muted",
                                    if center_enabled { {t!("yes")} } else { {t!("no")} }
                                }
                                dt { class: "font-medium text-fg-strong", {t!("skill-center-detail-created")} }
                                dd { class: "text-fg-muted", "{center.created_at}" }
                                dt { class: "font-medium text-fg-strong", {t!("skill-center-detail-updated")} }
                                dd { class: "text-fg-muted", "{center.updated_at}" }
                            }

                            // Cached catalog summary
                            div { class: "flex items-center justify-between mt-6 mb-3",
                                SectionHeading { class: "mb-0", {t!("skill-center-detail-catalog")} }
                                {
                                    let is_syncing = *syncing.read();
                                    let sync_id = id3.clone();
                                    rsx! {
                                        Button {
                                            size: ButtonSize::Md,
                                            disabled: is_syncing,
                                            onclick: move |_| {
                                                let sid = sync_id.clone();
                                                spawn(async move {
                                                    syncing.set(true);
                                                    sync_error.set(None);
                                                    match sid.parse::<uuid::Uuid>() {
                                                        Ok(id) => {
                                                            match sync_skill_center_now(SkillCenterSyncInput { id }).await {
                                                                Ok(_) => { catalog_summary.restart(); }
                                                                Err(e) => { sync_error.set(Some(e.to_string())); }
                                                            }
                                                        }
                                                        Err(e) => { sync_error.set(Some(e.to_string())); }
                                                    }
                                                    syncing.set(false);
                                                });
                                            },
                                            if is_syncing { {t!("skill-center-detail-syncing")} } else { {t!("skill-center-detail-sync-now")} }
                                        }
                                    }
                                }
                            }
                            if let Some(err) = &*sync_error.read() {
                                ErrorText { class: "mb-2", "{err}" }
                            }
                            {match &*catalog_summary.read() {
                                Some(Ok(summary)) => {
                                    if summary.skill_channels == 0 && summary.bundles == 0 && summary.mcp_servers == 0 && summary.mcp_bundles == 0 {
                                        rsx! { HelpText { {t!("skill-center-detail-no-catalog")} } }
                                    } else {
                                        rsx! {
                                            div { class: "grid grid-cols-2 sm:grid-cols-4 gap-4",
                                                StatTile { value: summary.skill_channels.to_string(), label: t!("skill-center-detail-skill-channels") }
                                                StatTile { value: summary.bundles.to_string(), label: t!("skill-center-detail-bundles") }
                                                StatTile { value: summary.mcp_servers.to_string(), label: t!("skill-center-detail-mcp-servers") }
                                                StatTile { value: summary.mcp_bundles.to_string(), label: t!("skill-center-detail-mcp-bundles") }
                                            }
                                            if let Some(ref ts) = summary.fetched_at {
                                                p { class: "help-xs mt-2", {t!("skill-center-detail-last-synced", time: ts.clone())} }
                                            }
                                        }
                                    }
                                },
                                Some(Err(e)) => rsx! { ErrorText { {t!("skill-center-detail-catalog-error", error: e.to_string())} } },
                                None => rsx! { HelpText { {t!("skill-center-detail-loading-catalog")} } },
                            }}
                        }
                    }
                },
                Some(Ok(None)) => rsx! { ErrorText { {t!("skill-center-detail-not-found")} } },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { HelpText { {t!("loading")} } },
            }
        }
    }
}

#[component]
fn StatTile(value: String, label: String) -> Element {
    rsx! {
        div { class: "bg-surface-2 rounded p-3",
            p { class: "text-2xl font-bold text-fg-strong", "{value}" }
            p { class: "help-xs", "{label}" }
        }
    }
}
