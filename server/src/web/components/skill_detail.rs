use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::anthropic::{GenerateContext, GeneratedNameDesc};
use crate::api_mcp::endpoints::skills::{
    ChannelMcpDepAddInput, ChannelMcpDepRemoveInput, ChannelMcpDepsInput, ChannelNixPackageAddInput,
    ChannelNixPackageRemoveInput, ChannelNixPackagesInput, ChannelPathsInput, McpServerOptionsInput,
    SkillChannelsInput, SkillGetInput, SkillUpdateInput, add_channel_mcp_dep,
    add_channel_nix_package, get_channel_nix_packages, get_skill, list_all_mcp_servers,
    list_channel_mcp_deps, list_channels, remove_channel_mcp_dep, remove_channel_nix_package,
    resolve_channel_paths, update_skill,
};
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::hidden_badge::HiddenBadge;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonKind, ButtonSize, ErrorText, HelpText, SectionHeading,
};

/// Parse a route-string id into a Uuid, mapping errors for server futures.
fn parse_id(id: &str) -> Result<uuid::Uuid, ServerFnError> {
    id.parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))
}

#[component]
pub fn SkillDetail(id: String) -> Element {
    use_topbar(t!("nav-skills"), None);
    let id_clone = id.clone();
    let mut skill = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_skill(SkillGetInput { id: parse_id(&id)? }).await }
    })?;

    let id_channels = id.clone();
    let channels = use_server_future(move || {
        let id = id_channels.clone();
        async move { list_channels(SkillChannelsInput { id: parse_id(&id)? }).await }
    })?;

    let id_paths = id.clone();
    let paths = use_server_future(move || {
        let id = id_paths.clone();
        async move { resolve_channel_paths(ChannelPathsInput { id: parse_id(&id)? }).await }
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut draft_hide = use_signal(|| false);

    match &*skill.read() {
        Some(Ok(s)) => {
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let sid = s.id.to_string();
            let skill_uuid = s.id;
            let name = s.name.clone();
            let desc = s.description.clone();
            let slug = s.slug.clone();
            let hide_flag = s.hide_from_public_catalog;

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    if *editing.read() {
                        form { class: "space-y-2",
                            onsubmit: move |evt: FormEvent| {
                                evt.prevent_default();
                                let new_name = draft_name.read().clone();
                                let new_desc = draft_desc.read().clone();
                                let new_hide = *draft_hide.read();
                                spawn(async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = update_skill(SkillUpdateInput {
                                            id: skill_uuid,
                                            name: new_name,
                                            description: new_desc,
                                            hide_from_public_catalog: new_hide,
                                        })
                                        .await;
                                        skill.restart();
                                    }
                                    editing.set(false);
                                });
                            },
                            input {
                                class: "input text-2xl font-bold py-1",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            textarea {
                                class: "input py-1",
                                rows: "2",
                                value: "{draft_desc}",
                                oninput: move |e| draft_desc.set(e.value()),
                            }
                            label { class: "flex items-center gap-2 text-sm text-fg-strong",
                                input {
                                    r#type: "checkbox",
                                    checked: "{draft_hide}",
                                    class: "rounded border-line",
                                    oninput: move |e| draft_hide.set(e.value() == "true"),
                                }
                                {t!("skill-detail-hide")}
                            }
                            div { class: "flex gap-2",
                                button { class: "text-success hover:opacity-80", r#type: "submit",
                                    {t!("save")}
                                }
                                button { class: "text-fg-muted hover:text-fg-strong", r#type: "button",
                                    onclick: move |_| editing.set(false),
                                    {t!("cancel")}
                                }
                                GenerateButton {
                                    context: GenerateContext::Skill { skill_id: sid.clone() },
                                    current_name: draft_name.read().clone(),
                                    current_desc: draft_desc.read().clone(),
                                    on_generated: move |result: GeneratedNameDesc| {
                                        draft_name.set(result.name);
                                        draft_desc.set(result.description);
                                    },
                                }
                            }
                        }
                    } else {
                        h1 { class: "h-page mb-0", "{name}" }
                        span { class: "text-fg-faint font-mono text-sm", "({slug})" }
                        HiddenBadge { hidden: hide_flag }
                        button { class: "text-fg-faint hover:text-fg-muted",
                            onclick: move |_| {
                                draft_name.set(name.clone());
                                draft_desc.set(desc.clone());
                                draft_hide.set(hide_flag);
                                editing.set(true);
                            },
                            {t!("edit")}
                        }
                    }
                }
                if !*editing.read() && !s.description.is_empty() {
                    p { class: "text-fg mb-2", "{s.description}" }
                }
                p { class: "help mb-6", {t!("cluster-detail-created", date: created)} }

                // Channels section (read-only, synced from xzar)
                div {
                    SectionHeading { {t!("skill-detail-channels")} }
                    p { class: "help-xs mb-3", {t!("skill-detail-channels-synced")} }
                    {match &*channels.read() {
                        Some(Ok(list)) if list.is_empty() => rsx! {
                            HelpText { {t!("skill-detail-no-channels")} }
                        },
                        Some(Ok(list)) => {
                            let path_map = match &*paths.read() {
                                Some(Ok(m)) => m.clone(),
                                _ => HashMap::new(),
                            };
                            rsx! {
                                ul { class: "divide-y divide-line-soft",
                                    for ch in list {
                                        {
                                            let ch_name = ch.channel.clone();
                                            let ch_created = ch.created_at.format("%Y-%m-%d").to_string();
                                            let arch_paths = path_map.get(&ch.channel).cloned().unwrap_or_default();
                                            let ch_id = ch.id.to_string();
                                            rsx! {
                                                li { class: "py-2",
                                                    div {
                                                        span { class: "text-sm font-mono font-medium", "{ch_name}" }
                                                        span { class: "text-xs text-fg-faint ml-2", "{ch_created}" }
                                                    }
                                                    for ap in &arch_paths {
                                                        p { class: "text-xs text-fg-faint font-mono mt-0.5 truncate",
                                                            span { class: "text-fg-muted", "{ap.arch}" }
                                                            " {ap.path}"
                                                        }
                                                    }
                                                    ChannelNixPackages { channel_id: ch_id.clone() }
                                                    ChannelMcpDeps { channel_id: ch_id }
                                                }
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

#[component]
fn ChannelMcpDeps(channel_id: String) -> Element {
    let cid = channel_id.clone();
    let mut deps = use_server_future(move || {
        let id = cid.clone();
        async move {
            list_channel_mcp_deps(ChannelMcpDepsInput {
                skill_channel_id: parse_id(&id)?,
            })
            .await
        }
    })?;

    let cid_add = channel_id.clone();
    let all_servers =
        use_server_future(move || async move { list_all_mcp_servers(McpServerOptionsInput {}).await })?;

    let mut selected_server = use_signal(String::new);

    rsx! {
        div { class: "mt-3",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("skill-detail-mcp-deps")} }
            form { class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let sid = selected_server.read().clone();
                    let channel = cid_add.clone();
                    spawn(async move {
                        if sid.is_empty() {
                            return;
                        }
                        let (Ok(skill_channel_id), Ok(mcp_server_id)) =
                            (channel.parse::<uuid::Uuid>(), sid.parse::<uuid::Uuid>())
                        else {
                            return;
                        };
                        if add_channel_mcp_dep(ChannelMcpDepAddInput {
                            skill_channel_id,
                            mcp_server_id,
                        })
                        .await
                        .is_ok()
                        {
                            selected_server.set(String::new());
                            deps.restart();
                        }
                    });
                },
                select { class: "input flex-1 w-auto py-1 text-sm",
                    value: "{selected_server}",
                    onchange: move |e| selected_server.set(e.value()),
                    option { value: "", {t!("skill-detail-select-mcp")} }
                    {match &*all_servers.read() {
                        Some(Ok(servers)) => rsx! {
                            for s in servers {
                                option { value: "{s.id}", "{s.slug} — {s.name}" }
                            }
                        },
                        _ => rsx! {},
                    }}
                }
                Button { kind: ButtonKind::Submit, size: ButtonSize::Xs,
                    {t!("add")}
                }
            }
            {match &*deps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { xs: true, {t!("skill-detail-no-mcp-deps")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for dep in list {
                            {
                                let dep_id = dep.dep_id;
                                let slug = dep.mcp_server_slug.clone();
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{slug}" }
                                        button { class: "link-danger text-xs",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    if remove_channel_mcp_dep(ChannelMcpDepRemoveInput { dep_id })
                                                        .await
                                                        .is_ok()
                                                    {
                                                        deps.restart();
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
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { HelpText { xs: true, {t!("loading")} } },
            }}
        }
    }
}

// ── Channel nix packages sub-component ────────────────────────────────

#[component]
fn ChannelNixPackages(channel_id: String) -> Element {
    let cid = channel_id.clone();
    let mut pkgs = use_server_future(move || {
        let id = cid.clone();
        async move {
            get_channel_nix_packages(ChannelNixPackagesInput {
                channel_id: parse_id(&id)?,
            })
            .await
        }
    })?;

    let mut new_pkg = use_signal(String::new);

    rsx! {
        div { class: "mt-3",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("skill-detail-nix-deps")} }
            form { class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let pkg = new_pkg.read().clone();
                    let ch = channel_id.clone();
                    spawn(async move {
                        let Ok(channel_id) = ch.parse::<uuid::Uuid>() else {
                            return;
                        };
                        if !pkg.trim().is_empty()
                            && add_channel_nix_package(ChannelNixPackageAddInput {
                                channel_id,
                                package: pkg,
                            })
                            .await
                            .is_ok()
                        {
                            new_pkg.set(String::new());
                            pkgs.restart();
                        }
                    });
                },
                input { class: "input flex-1 w-auto py-1 text-sm font-mono",
                    r#type: "text",
                    placeholder: t!("skill-detail-nix-placeholder"),
                    value: "{new_pkg}",
                    oninput: move |e| new_pkg.set(e.value()),
                }
                Button { kind: ButtonKind::Submit, size: ButtonSize::Xs,
                    {t!("add")}
                }
            }
            {match &*pkgs.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { xs: true, {t!("skill-detail-no-nix-deps")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for pkg in list {
                            {
                                let pkg_name = pkg.clone();
                                let ch = channel_id.clone();
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{pkg_name}" }
                                        button { class: "link-danger text-xs",
                                            onclick: move |_| {
                                                let p = pkg_name.clone();
                                                let c = ch.clone();
                                                spawn(async move {
                                                    let Ok(channel_id) = c.parse::<uuid::Uuid>() else {
                                                        return;
                                                    };
                                                    if remove_channel_nix_package(ChannelNixPackageRemoveInput {
                                                        channel_id,
                                                        package: p,
                                                    })
                                                    .await
                                                    .is_ok()
                                                    {
                                                        pkgs.restart();
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
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { HelpText { xs: true, {t!("loading")} } },
            }}
        }
    }
}
