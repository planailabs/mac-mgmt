use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::anthropic::{GenerateContext, GeneratedNameDesc};
use crate::api_mcp::endpoints::mcp_servers::{
    DependentSkillsInput, McpServerDeleteInput, McpServerGetInput, McpServerUpsertInput,
    NixPackageAddInput, NixPackageRemoveInput, add_nix_package, delete_mcp_server, get_mcp_server,
    list_dependent_skills, remove_nix_package, upsert_mcp_server,
};
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::hidden_badge::HiddenBadge;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonKind, ButtonSize, ErrorText, FormField, HelpText, PageHeader, SectionHeading,
};

// ── Shared form fields component ─────────────────────────────────────

#[component]
fn McpServerFormFields(
    slug: Signal<String>,
    name: Signal<String>,
    description: Signal<String>,
    config_json: Signal<String>,
    hide_from_public_catalog: Signal<bool>,
    slug_readonly: bool,
) -> Element {
    let slug_extra_class = if slug_readonly {
        "bg-surface-2 text-fg-muted"
    } else {
        ""
    };
    rsx! {
        FormField { label: t!("slug"),
            input {
                class: "input font-mono {slug_extra_class}",
                r#type: "text",
                required: true,
                readonly: slug_readonly,
                placeholder: t!("mcp-server-slug-placeholder"),
                value: "{slug}",
                oninput: move |evt| slug.set(evt.value()),
            }
        }
        FormField { label: t!("name"),
            input {
                class: "input",
                r#type: "text",
                required: true,
                value: "{name}",
                oninput: move |evt| name.set(evt.value()),
            }
        }
        FormField { label: t!("description"),
            textarea {
                class: "input",
                rows: "2",
                value: "{description}",
                oninput: move |evt| description.set(evt.value()),
            }
        }
        FormField { label: t!("mcp-server-config-json"),
            textarea {
                class: "input font-mono text-sm",
                rows: "10",
                required: true,
                value: "{config_json}",
                oninput: move |evt| config_json.set(evt.value()),
            }
        }
        div { class: "mb-4",
            label { class: "flex items-center gap-2 text-sm text-fg-strong",
                input {
                    r#type: "checkbox",
                    checked: "{hide_from_public_catalog}",
                    class: "rounded border-line",
                    oninput: move |evt| hide_from_public_catalog.set(evt.value() == "true"),
                }
                {t!("mcp-server-hide")}
            }
        }
    }
}

// ── Detail page (read-only + nix packages) ───────────────────────────

#[component]
pub fn McpServerDetail(id: String) -> Element {
    use_topbar(t!("nav-mcp-servers"), None);
    let navigator = navigator();
    let id_clone = id.clone();
    let mut server = use_server_future(move || {
        let id = id_clone.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_mcp_server(McpServerGetInput { id }).await
        }
    })?;

    let mut new_pkg = use_signal(String::new);

    let id_deps = id.clone();
    let dep_skills = use_server_future(move || {
        let id = id_deps.clone();
        async move {
            let mcp_server_id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_dependent_skills(DependentSkillsInput { mcp_server_id }).await
        }
    })?;

    match &*server.read() {
        Some(Ok(s)) => {
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let sid = s.id.to_string();
            let sid_del = s.id;
            let sid_pkg = s.id;
            let sid_rm = s.id;
            let name = s.name.clone();
            let slug = s.slug.clone();
            let config_str = serde_json::to_string_pretty(&s.config_json).unwrap_or_default();
            let packages = s.nix_packages.clone();
            let hide_flag = s.hide_from_public_catalog;

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    h2 { class: "h-page mb-0", "{name}" }
                    span { class: "text-fg-faint font-mono text-sm", "({slug})" }
                    HiddenBadge { hidden: hide_flag }
                    Link { to: Route::McpServerEdit { id: sid }, class: "text-fg-faint hover:text-fg-muted",
                        {t!("edit")}
                    }
                    button { class: "link-danger",
                        onclick: move |_| {
                            let id = sid_del;
                            let nav = navigator;
                            spawn(async move {
                                if delete_mcp_server(McpServerDeleteInput { id }).await.is_ok() {
                                    nav.push(Route::McpServerList {});
                                }
                            });
                        },
                        {t!("delete")}
                    }
                }
                if !s.description.is_empty() {
                    p { class: "text-fg mb-2", "{s.description}" }
                }
                p { class: "help mb-6", {t!("cluster-detail-created", date: created)} }

                div { class: "mb-6",
                    SectionHeading { {t!("mcp-server-config-json")} }
                    pre { class: "bg-surface-2 p-4 rounded text-sm font-mono overflow-x-auto whitespace-pre-wrap",
                        "{config_str}"
                    }
                }

                // Nix packages section
                div {
                    SectionHeading { {t!("mcp-server-nix-deps")} }
                    form { class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let id = sid_pkg;
                            let pkg = new_pkg.read().clone();
                            spawn(async move {
                                if !pkg.trim().is_empty()
                                    && add_nix_package(NixPackageAddInput { id, package: pkg })
                                        .await
                                        .is_ok()
                                {
                                    new_pkg.set(String::new());
                                    server.restart();
                                }
                            });
                        },
                        input { class: "input flex-1 w-auto py-1 text-sm font-mono",
                            r#type: "text",
                            placeholder: t!("mcp-server-nix-placeholder"),
                            value: "{new_pkg}",
                            oninput: move |e| new_pkg.set(e.value()),
                        }
                        Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                            {t!("add")}
                        }
                    }
                    if packages.is_empty() {
                        HelpText { {t!("mcp-server-no-nix-deps")} }
                    } else {
                        ul { class: "divide-y divide-line-soft",
                            for pkg in &packages {
                                {
                                    let pkg_display = pkg.clone();
                                    let pkg_remove = pkg.clone();
                                    rsx! {
                                        li { class: "py-2 flex justify-between items-center",
                                            span { class: "text-sm font-mono", "{pkg_display}" }
                                            button { class: "link-danger text-xs",
                                                onclick: move |_| {
                                                    let id = sid_rm;
                                                    let pkg = pkg_remove.clone();
                                                    spawn(async move {
                                                        if remove_nix_package(NixPackageRemoveInput { id, package: pkg }).await.is_ok() {
                                                            server.restart();
                                                        }
                                                    });
                                                },
                                                {t!("remove")}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Required by Skills section
                div { class: "mt-6",
                    SectionHeading { {t!("mcp-server-required-by")} }
                    {match &*dep_skills.read() {
                        Some(Ok(list)) if list.is_empty() => rsx! {
                            HelpText { {t!("mcp-server-no-skills-depend")} }
                        },
                        Some(Ok(list)) => rsx! {
                            ul { class: "divide-y divide-line-soft",
                                for dep in list {
                                    {
                                        let skill_slug = dep.skill_slug.clone();
                                        let channel = dep.channel.clone();
                                        let skill_id = dep.skill_id.clone();
                                        rsx! {
                                            li { class: "py-2",
                                                Link { to: Route::SkillDetail { id: skill_id }, class: "link text-sm font-mono",
                                                    "{skill_slug}"
                                                }
                                                span { class: "text-xs text-fg-faint ml-2", "({channel})" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                        None => rsx! { HelpText { {t!("loading")} } },
                    }}
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

// ── Create form ──────────────────────────────────────────────────────

#[component]
pub fn McpServerForm() -> Element {
    let navigator = navigator();
    let slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let config_json = use_signal(|| r#"{"command": "", "args": []}"#.to_string());
    let hide_from_public_catalog = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        PageHeader { {t!("mcp-server-new-title")} }
        if let Some(err) = &*error.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        form {
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                let nav = navigator;
                let s = slug.read().clone();
                let n = name.read().clone();
                let d = description.read().clone();
                let c = config_json.read().clone();
                let h = *hide_from_public_catalog.read();
                spawn(async move {
                    let input = McpServerUpsertInput {
                        id: None,
                        slug: s,
                        name: n,
                        description: d,
                        config_json: c,
                        hide_from_public_catalog: h,
                    };
                    match upsert_mcp_server(input).await {
                        Ok(server) => { nav.push(Route::McpServerDetail { id: server.id.to_string() }); }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            McpServerFormFields {
                slug, name, description, config_json, hide_from_public_catalog,
                slug_readonly: false,
            }
            div { class: "flex gap-3 items-center",
                Button { kind: ButtonKind::Submit, {t!("create")} }
                GenerateButton {
                    context: GenerateContext::McpServer { slug: slug.read().clone(), config_json: config_json.read().clone() },
                    current_name: name.read().clone(),
                    current_desc: description.read().clone(),
                    on_generated: move |result: GeneratedNameDesc| {
                        name.set(result.name);
                        description.set(result.description);
                    },
                }
            }
        }
    }
}

// ── Edit form ────────────────────────────────────────────────────────

#[component]
pub fn McpServerEdit(id: String) -> Element {
    let id_load = id.clone();
    let existing = use_server_future(move || {
        let id = id_load.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_mcp_server(McpServerGetInput { id }).await
        }
    })?;

    let navigator = navigator();
    let mut slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut config_json = use_signal(String::new);
    let mut hide_from_public_catalog = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut loaded = use_signal(|| false);

    if let Some(Ok(s)) = &*existing.read() {
        if !*loaded.read() {
            slug.set(s.slug.clone());
            name.set(s.name.clone());
            description.set(s.description.clone());
            config_json.set(serde_json::to_string_pretty(&s.config_json).unwrap_or_default());
            hide_from_public_catalog.set(s.hide_from_public_catalog);
            loaded.set(true);
        }
    }

    match &*existing.read() {
        Some(Ok(_)) => {
            let edit_id = id.clone();
            let nav_id = id.clone();
            rsx! {
                PageHeader { {t!("mcp-server-edit-title")} }
                if let Some(err) = &*error.read() {
                    ErrorText { class: "mb-4", "{err}" }
                }
                form {
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let nav = navigator;
                        let eid = edit_id.clone();
                        let nid = nav_id.clone();
                        let s = slug.read().clone();
                        let n = name.read().clone();
                        let d = description.read().clone();
                        let c = config_json.read().clone();
                        let h = *hide_from_public_catalog.read();
                        spawn(async move {
                            let Ok(eid) = eid.parse::<uuid::Uuid>() else {
                                error.set(Some("invalid mcp server id".to_string()));
                                return;
                            };
                            let input = McpServerUpsertInput {
                                id: Some(eid),
                                slug: s,
                                name: n,
                                description: d,
                                config_json: c,
                                hide_from_public_catalog: h,
                            };
                            match upsert_mcp_server(input).await {
                                Ok(_) => { nav.push(Route::McpServerDetail { id: nid }); }
                                Err(e) => error.set(Some(e.to_string())),
                            }
                        });
                    },
                    McpServerFormFields {
                        slug, name, description, config_json, hide_from_public_catalog,
                        slug_readonly: true,
                    }
                    div { class: "flex gap-3 items-center",
                        Button { kind: ButtonKind::Submit, {t!("save")} }
                        Link { to: Route::McpServerDetail { id: id.clone() },
                            class: "px-4 py-2 text-fg-muted hover:text-fg-strong",
                            {t!("cancel")}
                        }
                        GenerateButton {
                            context: GenerateContext::McpServer { slug: slug.read().clone(), config_json: config_json.read().clone() },
                            current_name: name.read().clone(),
                            current_desc: description.read().clone(),
                            on_generated: move |result: GeneratedNameDesc| {
                                name.set(result.name);
                                description.set(result.description);
                            },
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
