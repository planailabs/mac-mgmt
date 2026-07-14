use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::mcp_servers::{
    ClusterMcpBundleAttachInput, ClusterMcpBundleDetachInput, ClusterMcpBundlesInput,
    ClusterMcpServerAttachInput, ClusterMcpServerDetachInput, ClusterMcpServersInput,
    McpBundleOptionsAllInput, McpServerOptionsAllInput, RemoteMcpBundleOptionEntry,
    RemoteMcpBundleOptionsInput, RemoteMcpServerOptionEntry, RemoteMcpServerOptionsInput,
    add_cluster_mcp_bundle, add_cluster_mcp_server, list_all_mcp_bundles, list_all_mcp_servers,
    list_bundle_mcp_servers, list_cluster_mcp_bundles, list_cluster_mcp_servers,
    list_remote_mcp_bundle_options, list_remote_mcp_server_options, list_transitive_mcp_servers,
    remove_cluster_mcp_bundle, remove_cluster_mcp_server,
};
use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};

#[component]
pub fn ClusterMcpServers(cluster_id: String, read_only: bool) -> Element {
    let cid_servers = cluster_id.clone();
    let mut servers = use_server_future(move || {
        let cid = cid_servers.clone();
        async move {
            let cluster_id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_cluster_mcp_servers(ClusterMcpServersInput { cluster_id }).await
        }
    })?;

    let cid_bundles = cluster_id.clone();
    let mut bundles = use_server_future(move || {
        let cid = cid_bundles.clone();
        async move {
            let cluster_id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_cluster_mcp_bundles(ClusterMcpBundlesInput { cluster_id }).await
        }
    })?;

    let cid_bmcps = cluster_id.clone();
    let bundle_mcps = use_server_future(move || {
        let cid = cid_bmcps.clone();
        async move {
            let cluster_id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_bundle_mcp_servers(ClusterMcpServersInput { cluster_id }).await
        }
    })?;

    let cid_tmcps = cluster_id.clone();
    let transitive_mcps = use_server_future(move || {
        let cid = cid_tmcps.clone();
        async move {
            let cluster_id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_transitive_mcp_servers(ClusterMcpServersInput { cluster_id }).await
        }
    })?;

    let available_servers = use_server_future(|| async move {
        list_all_mcp_servers(McpServerOptionsAllInput {}).await
    })?;
    let available_bundles = use_server_future(|| async move {
        list_all_mcp_bundles(McpBundleOptionsAllInput {}).await
    })?;
    let available_remote_servers = use_server_future(|| async move {
        list_remote_mcp_server_options(RemoteMcpServerOptionsInput {}).await
    })?;
    let available_remote_mcp_bundles = use_server_future(|| async move {
        list_remote_mcp_bundle_options(RemoteMcpBundleOptionsInput {}).await
    })?;

    let mut selected_server = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_server = cluster_id.clone();
    let cid_add_bundle = cluster_id.clone();

    rsx! {
        // Direct MCP server assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("cluster-mcp-direct")} }
            if !read_only {
                form {
                    class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_server.clone();
                        let val = selected_server.read().clone();
                        spawn(async move {
                            if val.is_empty() {
                                return;
                            }
                            let Ok(cluster_id) = cid.parse::<uuid::Uuid>() else {
                                return;
                            };
                            if let Some(rest) = val.strip_prefix("remote|") {
                                let parts: Vec<&str> = rest.splitn(4, '|').collect();
                                if parts.len() == 4 {
                                    let (Ok(sc_id), Ok(remote_id)) = (
                                        parts[0].parse::<uuid::Uuid>(),
                                        parts[1].parse::<uuid::Uuid>(),
                                    ) else {
                                        return;
                                    };
                                    let input = ClusterMcpServerAttachInput {
                                        cluster_id,
                                        mcp_server_id: None,
                                        skill_center_id: Some(sc_id),
                                        remote_id: Some(remote_id),
                                        slug: Some(parts[2].to_string()),
                                        mcp_name: Some(parts[3].to_string()),
                                    };
                                    if add_cluster_mcp_server(input).await.is_ok() {
                                        selected_server.set(String::new());
                                        servers.restart();
                                    }
                                }
                            } else {
                                let Ok(mcp_server_id) = val.parse::<uuid::Uuid>() else {
                                    return;
                                };
                                let input = ClusterMcpServerAttachInput {
                                    cluster_id,
                                    mcp_server_id: Some(mcp_server_id),
                                    skill_center_id: None,
                                    remote_id: None,
                                    slug: None,
                                    mcp_name: None,
                                };
                                if add_cluster_mcp_server(input).await.is_ok() {
                                    selected_server.set(String::new());
                                    servers.restart();
                                }
                            }
                        });
                    },
                    select { class: "input flex-1 w-auto py-1 text-sm",
                        value: "{selected_server}",
                        onchange: move |evt| selected_server.set(evt.value()),
                        option { value: "", {t!("cluster-mcp-select")} }
                        {match &*available_servers.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-mcp-local"),
                                    for s in list {
                                        {
                                            let val = s.id.to_string();
                                            let label = format!("{} ({})", s.name, s.slug);
                                            rsx! { option { value: "{val}", "{label}" } }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                        {match &*available_remote_servers.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteMcpServerOptionEntry>> = std::collections::BTreeMap::new();
                                for rm in list.iter() {
                                    by_sc.entry(rm.skill_center_name.clone()).or_default().push(rm);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-mcp-from-sc", name: sc_name.clone()),
                                            for rm in items {
                                                {
                                                    let val = format!(
                                                        "remote|{}|{}|{}|{}",
                                                        rm.skill_center_id, rm.remote_mcp_server_id,
                                                        rm.slug, rm.name
                                                    );
                                                    let label = format!("{} ({})", rm.name, rm.slug);
                                                    rsx! { option { value: "{val}", "{label}" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                    }
                    Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                        {t!("add")}
                    }
                }
            }
            {match &*servers.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-direct")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for cs in list {
                            {
                                let csid = cs.cluster_mcp_server_id;
                                let label = format!("{} ({})", cs.server_name, cs.server_slug);
                                let is_remote = cs.skill_center_name.is_some();
                                let via = cs.skill_center_name.clone().unwrap_or_default();
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "flex items-center gap-2",
                                            span {
                                                class: if is_remote { "text-sm font-mono text-fg-muted" } else { "text-sm font-mono" },
                                                "{label}"
                                            }
                                            if is_remote {
                                                span { class: "text-xs text-fg-faint", {t!("cluster-mcp-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button { class: "link-danger text-sm",
                                                onclick: move |_| {
                                                    let cluster_mcp_server_id = csid;
                                                    spawn(async move {
                                                        if remove_cluster_mcp_server(ClusterMcpServerDetachInput { cluster_mcp_server_id }).await.is_ok() {
                                                            servers.restart();
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
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }

        // MCP servers from bundles (read-only, blue)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-info mb-2", {t!("cluster-mcp-from-bundles")} }
            {match &*bundle_mcps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-bundle-mcp")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for bm in list {
                            {
                                let label = format!("{} ({})", bm.server_name, bm.server_slug);
                                let via = bm.bundle_slug.clone();
                                let overwritten = bm.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-info opacity-50 line-through" } else { "text-sm font-mono text-info" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-info opacity-50" } else { "text-xs text-info" }, {t!("cluster-mcp-via", source: via.clone())} }
                                        if overwritten {
                                            span { class: "text-xs text-fg-faint italic", {t!("cluster-mcp-overwritten")} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }

        // MCP servers from skills (transitive, read-only, grey)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-muted mb-2", {t!("cluster-mcp-from-skills")} }
            {match &*transitive_mcps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-transitive")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for tm in list {
                            {
                                let label = format!("{} ({})", tm.server_name, tm.server_slug);
                                let via = format!("{} / {}", tm.skill_slug, tm.channel);
                                let overwritten = tm.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-fg-muted opacity-50 line-through" } else { "text-sm font-mono text-fg-muted" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-fg-faint opacity-50" } else { "text-xs text-fg-faint" }, {t!("cluster-mcp-via", source: via.clone())} }
                                        if overwritten {
                                            span { class: "text-xs text-fg-faint italic", {t!("cluster-mcp-overwritten")} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }

        // MCP bundle assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("cluster-mcp-bundles-title")} }
            if !read_only {
                if let Some(err) = &*bundle_error.read() {
                    ErrorText { class: "mb-2", "{err}" }
                }
                form {
                    class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_bundle.clone();
                        let val = selected_bundle.read().clone();
                        spawn(async move {
                            if val.is_empty() {
                                return;
                            }
                            let Ok(cluster_id) = cid.parse::<uuid::Uuid>() else {
                                bundle_error.set(Some("invalid cluster id".to_string()));
                                return;
                            };
                            if let Some(rest) = val.strip_prefix("remote|") {
                                let parts: Vec<&str> = rest.splitn(4, '|').collect();
                                if parts.len() == 4 {
                                    let (Ok(sc_id), Ok(remote_id)) = (
                                        parts[0].parse::<uuid::Uuid>(),
                                        parts[1].parse::<uuid::Uuid>(),
                                    ) else {
                                        bundle_error.set(Some("invalid remote bundle id".to_string()));
                                        return;
                                    };
                                    let input = ClusterMcpBundleAttachInput {
                                        cluster_id,
                                        bundle_id: None,
                                        skill_center_id: Some(sc_id),
                                        remote_id: Some(remote_id),
                                        slug: Some(parts[2].to_string()),
                                        bundle_name: Some(parts[3].to_string()),
                                    };
                                    match add_cluster_mcp_bundle(input).await {
                                        Ok(()) => {
                                            bundle_error.set(None);
                                            selected_bundle.set(String::new());
                                            bundles.restart();
                                        }
                                        Err(e) => {
                                            bundle_error.set(Some(e.to_string()));
                                        }
                                    }
                                }
                            } else {
                                let Ok(bundle_id) = val.parse::<uuid::Uuid>() else {
                                    bundle_error.set(Some("invalid bundle id".to_string()));
                                    return;
                                };
                                let input = ClusterMcpBundleAttachInput {
                                    cluster_id,
                                    bundle_id: Some(bundle_id),
                                    skill_center_id: None,
                                    remote_id: None,
                                    slug: None,
                                    bundle_name: None,
                                };
                                match add_cluster_mcp_bundle(input).await {
                                    Ok(()) => {
                                        bundle_error.set(None);
                                        selected_bundle.set(String::new());
                                        bundles.restart();
                                    }
                                    Err(e) => {
                                        bundle_error.set(Some(e.to_string()));
                                    }
                                }
                            }
                        });
                    },
                    select { class: "input flex-1 w-auto py-1 text-sm",
                        value: "{selected_bundle}",
                        onchange: move |evt| selected_bundle.set(evt.value()),
                        option { value: "", {t!("cluster-mcp-select-bundle")} }
                        {match &*available_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-mcp-local"),
                                    for b in list {
                                        {
                                            let val = b.id.to_string();
                                            let label = format!("{} ({})", b.name, b.slug);
                                            rsx! { option { value: "{val}", "{label}" } }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                        {match &*available_remote_mcp_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteMcpBundleOptionEntry>> = std::collections::BTreeMap::new();
                                for rb in list.iter() {
                                    by_sc.entry(rb.skill_center_name.clone()).or_default().push(rb);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-mcp-from-sc", name: sc_name.clone()),
                                            for rb in items {
                                                {
                                                    let val = format!(
                                                        "remote|{}|{}|{}|{}",
                                                        rb.skill_center_id, rb.remote_bundle_id,
                                                        rb.slug, rb.name
                                                    );
                                                    let label = format!("{} ({})", rb.name, rb.slug);
                                                    rsx! { option { value: "{val}", "{label}" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                    }
                    Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                        {t!("add")}
                    }
                }
            }
            {match &*bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-bundle-assign")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for cb in list {
                            {
                                let cbid = cb.cluster_mcp_bundle_id;
                                let label = format!("{} ({})", cb.bundle_name, cb.bundle_slug);
                                let is_remote = cb.skill_center_name.is_some();
                                let via = cb.skill_center_name.clone().unwrap_or_default();
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "flex items-center gap-2",
                                            span {
                                                class: if is_remote { "text-sm text-fg-muted" } else { "text-sm" },
                                                "{label}"
                                            }
                                            if is_remote {
                                                span { class: "text-xs text-fg-faint", {t!("cluster-mcp-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button { class: "link-danger text-sm",
                                                onclick: move |_| {
                                                    let cluster_mcp_bundle_id = cbid;
                                                    spawn(async move {
                                                        if remove_cluster_mcp_bundle(ClusterMcpBundleDetachInput { cluster_mcp_bundle_id }).await.is_ok() {
                                                            bundles.restart();
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
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }
    }
}
