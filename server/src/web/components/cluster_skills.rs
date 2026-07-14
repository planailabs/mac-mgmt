use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::skills::{
    BundleOptionsInput, ClusterBundleAddInput, ClusterBundleRemoveInput, ClusterBundleSkillsInput,
    ClusterBundlesListInput, ClusterSkillAddInput, ClusterSkillRemoveInput, ClusterSkillsListInput,
    RemoteBundleOption, RemoteBundleOptionsInput, RemoteSkillOption, RemoteSkillOptionsInput,
    SkillChannelOptionsInput, add_cluster_bundle, add_cluster_skill, list_all_bundles,
    list_all_skill_channels, list_bundle_skills, list_cluster_bundles, list_cluster_skills,
    list_remote_bundle_options, list_remote_skill_options, remove_cluster_bundle,
    remove_cluster_skill,
};
use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};

/// Parse a route-string id into a Uuid, mapping errors for server futures.
fn parse_id(id: &str) -> Result<uuid::Uuid, ServerFnError> {
    id.parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))
}

#[component]
pub fn ClusterSkills(cluster_id: String, read_only: bool) -> Element {
    let cid_skills = cluster_id.clone();
    let mut skills = use_server_future(move || {
        let cid = cid_skills.clone();
        async move {
            list_cluster_skills(ClusterSkillsListInput {
                cluster_id: parse_id(&cid)?,
            })
            .await
        }
    })?;

    let cid_bundles = cluster_id.clone();
    let mut bundles = use_server_future(move || {
        let cid = cid_bundles.clone();
        async move {
            list_cluster_bundles(ClusterBundlesListInput {
                cluster_id: parse_id(&cid)?,
            })
            .await
        }
    })?;

    let cid_bskills = cluster_id.clone();
    let bundle_skills = use_server_future(move || {
        let cid = cid_bskills.clone();
        async move {
            list_bundle_skills(ClusterBundleSkillsInput {
                cluster_id: parse_id(&cid)?,
            })
            .await
        }
    })?;

    let available_sc = use_server_future(|| async move {
        list_all_skill_channels(SkillChannelOptionsInput {}).await
    })?;
    let available_bundles =
        use_server_future(|| async move { list_all_bundles(BundleOptionsInput {}).await })?;
    let available_remote_sc = use_server_future(|| async move {
        list_remote_skill_options(RemoteSkillOptionsInput {}).await
    })?;
    let available_remote_bundles = use_server_future(|| async move {
        list_remote_bundle_options(RemoteBundleOptionsInput {}).await
    })?;

    let mut selected_sc = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_skill = cluster_id.clone();
    let cid_add_bundle = cluster_id.clone();

    rsx! {
        // Direct skill assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("cluster-skills-direct")} }
            if !read_only {
                form { class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_skill.clone();
                        let val = selected_sc.read().clone();
                        spawn(async move {
                            if val.is_empty() {
                                return;
                            }
                            let Ok(cluster_id) = cid.parse::<uuid::Uuid>() else {
                                return;
                            };
                            if let Some(rest) = val.strip_prefix("remote|") {
                                let parts: Vec<&str> = rest.splitn(5, '|').collect();
                                if parts.len() != 5 {
                                    return;
                                }
                                let (Ok(skill_center_id), Ok(remote_id)) = (
                                    parts[0].parse::<uuid::Uuid>(),
                                    parts[1].parse::<uuid::Uuid>(),
                                ) else {
                                    return;
                                };
                                if add_cluster_skill(ClusterSkillAddInput {
                                    cluster_id,
                                    skill_channel_id: None,
                                    skill_center_id: Some(skill_center_id),
                                    remote_id: Some(remote_id),
                                    slug: Some(parts[2].to_string()),
                                    channel: Some(parts[3].to_string()),
                                    skill_name: Some(parts[4].to_string()),
                                })
                                .await
                                .is_ok()
                                {
                                    selected_sc.set(String::new());
                                    skills.restart();
                                }
                            } else {
                                let Ok(skill_channel_id) = val.parse::<uuid::Uuid>() else {
                                    return;
                                };
                                if add_cluster_skill(ClusterSkillAddInput {
                                    cluster_id,
                                    skill_channel_id: Some(skill_channel_id),
                                    skill_center_id: None,
                                    remote_id: None,
                                    slug: None,
                                    channel: None,
                                    skill_name: None,
                                })
                                .await
                                .is_ok()
                                {
                                    selected_sc.set(String::new());
                                    skills.restart();
                                }
                            }
                        });
                    },
                    select { class: "input flex-1 w-auto py-1 text-sm",
                        value: "{selected_sc}",
                        onchange: move |evt| selected_sc.set(evt.value()),
                        option { value: "", {t!("cluster-skills-select")} }
                        {match &*available_sc.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-skills-local"),
                                    for sc in list {
                                        {
                                            let val = sc.id.to_string();
                                            let label = format!("{} / {}", sc.skill_slug, sc.channel);
                                            rsx! { option { value: "{val}", "{label}" } }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                        {match &*available_remote_sc.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteSkillOption>> = std::collections::BTreeMap::new();
                                for rsc in list.iter() {
                                    by_sc.entry(rsc.skill_center_name.clone()).or_default().push(rsc);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-skills-from-sc", name: sc_name.clone()),
                                            for rsc in items {
                                                {
                                                    let val = format!(
                                                        "remote|{}|{}|{}|{}|{}",
                                                        rsc.skill_center_id, rsc.remote_skill_channel_id,
                                                        rsc.skill_slug, rsc.channel, rsc.skill_name
                                                    );
                                                    let label = format!("{} / {}", rsc.skill_slug, rsc.channel);
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
            {match &*skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-skills-no-direct")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for cs in list {
                            {
                                let csid = cs.cluster_skill_id;
                                let label = format!("{} / {}", cs.skill_slug, cs.channel);
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
                                                span { class: "text-xs text-fg-faint", {t!("cluster-skills-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button { class: "link-danger text-sm",
                                                onclick: move |_| {
                                                    spawn(async move {
                                                        if remove_cluster_skill(ClusterSkillRemoveInput {
                                                            cluster_skill_id: csid,
                                                        })
                                                        .await
                                                        .is_ok()
                                                        {
                                                            skills.restart();
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

        // Skills from bundles (read-only, info color)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-info mb-2", {t!("cluster-skills-from-bundles")} }
            {match &*bundle_skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-skills-no-bundle-skills")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for bs in list {
                            {
                                let label = format!("{} / {}", bs.skill_slug, bs.channel);
                                let via = bs.bundle_slug.clone();
                                let overwritten = bs.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-info opacity-50 line-through" } else { "text-sm font-mono text-info" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-info opacity-50" } else { "text-xs text-info" }, {t!("cluster-skills-via", source: via.clone())} }
                                        if overwritten {
                                            span { class: "text-xs text-fg-faint italic", {t!("cluster-skills-overwritten")} }
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

        // Bundle assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("cluster-skills-bundles-title")} }
            if let Some(err) = &*bundle_error.read() {
                ErrorText { class: "mb-2", "{err}" }
            }
            if !read_only {
                form { class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_bundle.clone();
                        let val = selected_bundle.read().clone();
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
                                    let (Ok(skill_center_id), Ok(remote_id)) = (
                                        parts[0].parse::<uuid::Uuid>(),
                                        parts[1].parse::<uuid::Uuid>(),
                                    ) else {
                                        return;
                                    };
                                    match add_cluster_bundle(ClusterBundleAddInput {
                                        cluster_id,
                                        bundle_id: None,
                                        skill_center_id: Some(skill_center_id),
                                        remote_id: Some(remote_id),
                                        slug: Some(parts[2].to_string()),
                                        bundle_name: Some(parts[3].to_string()),
                                    })
                                    .await
                                    {
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
                                    return;
                                };
                                match add_cluster_bundle(ClusterBundleAddInput {
                                    cluster_id,
                                    bundle_id: Some(bundle_id),
                                    skill_center_id: None,
                                    remote_id: None,
                                    slug: None,
                                    bundle_name: None,
                                })
                                .await
                                {
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
                        option { value: "", {t!("cluster-skills-select-bundle")} }
                        {match &*available_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-skills-local"),
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
                        {match &*available_remote_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteBundleOption>> = std::collections::BTreeMap::new();
                                for rb in list.iter() {
                                    by_sc.entry(rb.skill_center_name.clone()).or_default().push(rb);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-skills-from-sc", name: sc_name.clone()),
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
                    HelpText { {t!("cluster-skills-no-bundle-assign")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for cb in list {
                            {
                                let cbid = cb.cluster_bundle_id;
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
                                                span { class: "text-xs text-fg-faint", {t!("cluster-skills-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button { class: "link-danger text-sm",
                                                onclick: move |_| {
                                                    spawn(async move {
                                                        if remove_cluster_bundle(ClusterBundleRemoveInput {
                                                            cluster_bundle_id: cbid,
                                                        })
                                                        .await
                                                        .is_ok()
                                                        {
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
