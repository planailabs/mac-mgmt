use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::clusters::{
    ClusterCanWriteInput, ClusterGetInput, SecretCreateInput, SecretDeleteInput, SecretUpdateInput,
    SecretsListInput, can_write_cluster, create_secret, delete_secret, get_cluster, list_secrets,
    update_secret,
};
use crate::web::components::ui::{
    Button, ButtonSize, ErrorText, HelpText, PageHeader, SectionHeading,
};

use super::config_editor_panel::ConfigEditorPanel;
use super::config_history::ConfigHistory;
use super::topbar::use_topbar;

// ── Page component ──────────────────────────────────────────────────────

#[component]
pub fn ClusterConfigPage(id: String) -> Element {
    let cid_name = id.clone();
    let cluster_name = use_server_future(move || {
        let cid = cid_name.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            let cluster = get_cluster(ClusterGetInput { id }).await?;
            Ok::<String, ServerFnError>(cluster.name)
        }
    })?;

    let cid_write = id.clone();
    let write_check = use_server_future(move || {
        let cid = cid_write.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            can_write_cluster(ClusterCanWriteInput { id }).await
        }
    })?;

    let can_write = matches!(&*write_check.read(), Some(Ok(true)));
    let read_only = !can_write;

    let name = match &*cluster_name.read() {
        Some(Ok(n)) => n.clone(),
        _ => String::new(),
    };

    // Publish the cluster name to TopbarMeta so the breadcrumb's
    // `parent_dyn` for `ClusterDetail` resolves to the cluster's name
    // instead of repeating the static "Clusters" label twice.
    use_topbar(
        name.clone(),
        Some(t!("cluster-detail-tab-config").to_string()),
    );

    rsx! {
        // The trailing `pb-32` reserves vertical space below the last
        // section so the floating SaveBar (`fixed bottom-5 …`) never
        // overlaps Config History / Secrets / the final field row when
        // the user scrolls to the very bottom of the page.
        div { class: "pb-32",
            if !name.is_empty() {
                PageHeader { class: "mb-4", "{name} — {t!(\"cluster-detail-tab-config\")}" }
            }

            ConfigEditorPanel { cluster_id: id.clone(), read_only }

            // Secrets + history land below the editor as full-width
            // sections (the right rail handles drill-down navigation,
            // no need for a side-by-side grid here).
            div { class: "space-y-6 mt-10",
                div { id: "sec-secrets",
                    SectionHeading { {t!("secrets-title")} }
                    p { class: "help mb-3", {t!("secrets-description")} }
                    SecretsEditor { cluster_id: id.clone(), read_only }
                }
                div { id: "sec-config-history",
                    SectionHeading { {t!("cluster-detail-tab-config-history")} }
                    ConfigHistory { cluster_id: id.clone() }
                }
            }
        }
    }
}

// ── Secrets editor component ────────────────────────────────────────────

#[component]
fn SecretsEditor(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut secrets = use_server_future(move || {
        let cid = cid.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_secrets(SecretsListInput { id }).await
        }
    })?;

    let mut error = use_signal(|| None::<String>);
    let mut new_name = use_signal(String::new);
    let mut new_value = use_signal(String::new);
    let mut editing: Signal<Option<String>> = use_signal(|| None);
    let mut edit_value = use_signal(String::new);
    let mut confirm_delete: Signal<Option<String>> = use_signal(|| None);

    let entries = match &*secrets.read() {
        Some(Ok(list)) => list.clone(),
        Some(Err(e)) => {
            return rsx! { ErrorText { {t!("error-message", message: e.to_string())} } };
        }
        None => {
            return rsx! { HelpText { {t!("loading")} } };
        }
    };

    let cid_create = cluster_id.clone();
    let mut do_create = move || {
        let cid = cid_create.clone();
        let name = new_name.read().trim().to_string();
        let value = new_value.read().clone();
        if name.is_empty() || value.is_empty() {
            return;
        }
        let Ok(id) = cid.parse::<uuid::Uuid>() else {
            error.set(Some("invalid cluster id".to_string()));
            return;
        };
        spawn(async move {
            match create_secret(SecretCreateInput { id, name, value }).await {
                Ok(()) => {
                    error.set(None);
                    new_name.set(String::new());
                    new_value.set(String::new());
                    secrets.restart();
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    rsx! {
        if let Some(err) = &*error.read() {
            ErrorText { class: "mb-2", "{err}" }
        }

        if entries.is_empty() {
            div { class: "rounded-lg border border-dashed border-line bg-surface-2/40 px-5 py-6 mb-4 text-center",
                div { class: "text-fg-faint text-2xl mb-1", "🔑" }
                HelpText { class: "max-w-sm mx-auto", {t!("secrets-empty")} }
            }
        } else {
            // Card grid: secrets show as cards with key glyph, name,
            // reference (click-to-copy), created date and inline
            // edit/delete actions. Replaces the dense table with a
            // touch-friendly layout that scales to mobile.
            div { class: "grid gap-2 sm:grid-cols-2 mb-4",
                for entry in &entries {
                    {
                        let name = entry.name.clone();
                        let created = entry.created_at.clone();
                        let reference = format!("secret:{name}");
                        let reference_copy = reference.clone();
                        let name_edit = name.clone();
                        let name_del = name.clone();
                        let name_del2 = name.clone();
                        let cid_upd = cluster_id.clone();
                        let cid_del = cluster_id.clone();
                        let is_editing = editing.read().as_deref() == Some(name.as_str());
                        let is_confirming = confirm_delete.read().as_deref() == Some(name.as_str());

                        rsx! {
                            div { class: "card p-3 flex flex-col gap-2",
                                key: "{name}",
                                div { class: "flex items-start gap-3",
                                    span { class: "shrink-0 w-9 h-9 rounded-lg bg-brand-soft text-brand flex items-center justify-center text-base",
                                        "🔑"
                                    }
                                    div { class: "min-w-0 flex-1",
                                        div { class: "flex items-center justify-between gap-2",
                                            div { class: "font-mono text-sm text-fg-strong truncate", "{name}" }
                                            span { class: "kicker text-fg-faint shrink-0", "{created}" }
                                        }
                                        button {
                                            class: "mt-1 text-[11px] font-mono text-fg-muted hover:text-brand transition-colors flex items-center gap-1 truncate",
                                            r#type: "button",
                                            title: t!("secrets-click-to-copy"),
                                            onclick: move |_| {
                                                let r = reference_copy.clone();
                                                spawn(async move {
                                                    let _ = document::eval(&format!(
                                                        "navigator.clipboard.writeText({r:?}).catch(function(){{}});"
                                                    )).await;
                                                });
                                            },
                                            span { class: "select-all truncate", "{reference}" }
                                            span { class: "text-fg-faint shrink-0", "⧉" }
                                        }
                                    }
                                }
                                if !read_only {
                                    div { class: "border-t border-line-soft pt-2",
                                        if is_editing {
                                            div { class: "flex flex-wrap gap-1.5 items-center",
                                                input {
                                                    r#type: "password",
                                                    class: "input input-xs flex-1 min-w-[120px]",
                                                    placeholder: t!("secrets-new-value"),
                                                    value: "{edit_value}",
                                                    oninput: move |e| edit_value.set(e.value()),
                                                }
                                                button { r#type: "button",
                                                    class: "btn btn-xs btn-primary",
                                                    onclick: move |_| {
                                                        let cid = cid_upd.clone();
                                                        let n = name_edit.clone();
                                                        let v = edit_value.read().clone();
                                                        spawn(async move {
                                                            let Ok(id) = cid.parse::<uuid::Uuid>() else {
                                                                error.set(Some("invalid cluster id".to_string()));
                                                                return;
                                                            };
                                                            match update_secret(SecretUpdateInput { id, name: n, value: v }).await {
                                                                Ok(()) => {
                                                                    error.set(None);
                                                                    editing.set(None);
                                                                    edit_value.set(String::new());
                                                                    secrets.restart();
                                                                }
                                                                Err(e) => error.set(Some(e.to_string())),
                                                            }
                                                        });
                                                    },
                                                    {t!("save")}
                                                }
                                                button { r#type: "button",
                                                    class: "btn btn-xs btn-ghost",
                                                    onclick: move |_| {
                                                        editing.set(None);
                                                        edit_value.set(String::new());
                                                    },
                                                    {t!("cancel")}
                                                }
                                            }
                                        } else if is_confirming {
                                            div { class: "flex flex-wrap gap-1.5 items-center",
                                                span { class: "text-xs text-danger flex-1", {t!("secrets-confirm-delete")} }
                                                button { r#type: "button",
                                                    class: "btn btn-xs btn-danger",
                                                    onclick: move |_| {
                                                        let cid = cid_del.clone();
                                                        let n = name_del.clone();
                                                        spawn(async move {
                                                            let Ok(id) = cid.parse::<uuid::Uuid>() else {
                                                                error.set(Some("invalid cluster id".to_string()));
                                                                return;
                                                            };
                                                            match delete_secret(SecretDeleteInput { id, name: n }).await {
                                                                Ok(()) => {
                                                                    error.set(None);
                                                                    confirm_delete.set(None);
                                                                    secrets.restart();
                                                                }
                                                                Err(e) => error.set(Some(e.to_string())),
                                                            }
                                                        });
                                                    },
                                                    {t!("delete")}
                                                }
                                                button { r#type: "button",
                                                    class: "btn btn-xs btn-ghost",
                                                    onclick: move |_| confirm_delete.set(None),
                                                    {t!("cancel")}
                                                }
                                            }
                                        } else {
                                            div { class: "flex gap-3 items-center",
                                                button { r#type: "button",
                                                    class: "link text-xs",
                                                    onclick: move |_| {
                                                        editing.set(Some(name.clone()));
                                                        edit_value.set(String::new());
                                                    },
                                                    {t!("secrets-update")}
                                                }
                                                span { class: "text-fg-faint text-xs", "·" }
                                                button { r#type: "button",
                                                    class: "link-danger text-xs",
                                                    onclick: move |_| {
                                                        confirm_delete.set(Some(name_del2.clone()));
                                                    },
                                                    {t!("delete")}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if !read_only {
            div { class: "rounded-xl border border-line bg-surface-2/30 p-4",
                div { class: "flex items-center gap-2 mb-3",
                    span { class: "kicker", {t!("secrets-add")} }
                    span { class: "text-xs text-fg-muted", {t!("secrets-add-hint")} }
                }
                div { class: "grid gap-2 sm:grid-cols-[1fr_2fr_auto] items-end",
                    div { class: "flex flex-col gap-1",
                        label { class: "help-xs", {t!("secrets-col-name")} }
                        input {
                            r#type: "text",
                            class: "input input-sm font-mono",
                            placeholder: "MY_API_KEY",
                            value: "{new_name}",
                            oninput: move |e| new_name.set(e.value()),
                        }
                    }
                    div { class: "flex flex-col gap-1",
                        label { class: "help-xs", {t!("secrets-col-value")} }
                        input {
                            r#type: "password",
                            class: "input input-sm",
                            placeholder: t!("secrets-value-placeholder"),
                            value: "{new_value}",
                            oninput: move |e| new_value.set(e.value()),
                        }
                    }
                    Button { size: ButtonSize::Sm,
                        onclick: move |_| do_create(),
                        {t!("secrets-add")}
                    }
                }
            }
        }
    }
}
