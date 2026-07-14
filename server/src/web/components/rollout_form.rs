use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::rollouts::{
    AvailableVersionsInput, GroupOptionsInput, RolloutCreateInput, create_rollout,
    get_available_versions, get_group_options,
};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonSize, ErrorText, FormField, HelpText, PageHeader};
use crate::web::gate_input::HealthGateInput;

const ALL_CLUSTERS_SENTINEL: &str = "__all__";

#[component]
pub fn RolloutForm() -> Element {
    use_topbar(t!("nav-rollouts"), None);
    let groups =
        use_server_future(move || async move { get_group_options(GroupOptionsInput {}).await })?;
    let versions = use_server_future(move || async move {
        get_available_versions(AvailableVersionsInput {}).await
    })?;
    let mut name = use_signal(String::new);
    let mut target_version = use_signal(String::new);
    let mut nixpkgs_commit = use_signal(String::new);
    let mut selected_stages = use_signal(Vec::<String>::new);
    let mut ramp_minutes = use_signal(String::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut gate = use_signal(HealthGateInput::default);
    let nav = navigator();

    match &*groups.read() {
        Some(Ok(group_list)) => {
            let group_list_clone = group_list.clone();
            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    PageHeader { class: "mb-0", {t!("rollout-form-title")} }
                    Link { to: Route::RolloutGroupList {}, class: "link text-sm",
                        {t!("rollout-list-manage-groups")}
                    }
                }

                div { class: "space-y-4",
                    FormField { label: t!("rollout-form-name-label"), help: t!("rollout-form-name-help"),
                        input { class: "input",
                            placeholder: t!("rollout-form-name-placeholder"),
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                    }
                    FormField { label: t!("rollout-form-target-version"), help: t!("rollout-form-version-help"),
                        {
                            let version_list: Vec<String> = match &*versions.read() {
                                Some(Ok(v)) => v.clone(),
                                _ => Vec::new(),
                            };
                            rsx! {
                                select { class: "input font-mono",
                                    value: "{target_version}",
                                    onchange: move |e| target_version.set(e.value()),
                                    option { value: "", {t!("rollout-form-none-nixpkgs")} }
                                    for v in version_list.iter() {
                                        option { value: "{v}", "{v}" }
                                    }
                                }
                                if version_list.is_empty() {
                                    p { class: "text-xs text-warn-strong mt-1",
                                        {t!("rollout-form-no-versions")}
                                    }
                                }
                            }
                        }
                    }
                    FormField { label: t!("rollout-form-nixpkgs-label"), help: t!("rollout-form-nixpkgs-help"),
                        input { class: "input font-mono",
                            placeholder: t!("rollout-form-nixpkgs-placeholder"),
                            value: "{nixpkgs_commit}",
                            oninput: move |e| nixpkgs_commit.set(e.value()),
                        }
                    }
                    FormField { label: t!("rollout-form-ramp-label"), help: t!("rollout-form-ramp-help"),
                        input { class: "input",
                            r#type: "number",
                            min: "1",
                            placeholder: t!("rollout-form-ramp-placeholder"),
                            value: "{ramp_minutes}",
                            oninput: move |e| ramp_minutes.set(e.value()),
                        }
                    }
                    div {
                        label { class: "label", {t!("rollout-form-stages-label")} }

                        // "All Clusters" as a selectable stage
                        {
                            let key = ALL_CLUSTERS_SENTINEL.to_string();
                            let is_selected = selected_stages.read().contains(&key);
                            let order = selected_stages.read().iter().position(|x| x == &key);
                            rsx! {
                                div { class: "flex items-center gap-2 mb-1",
                                    input {
                                        r#type: "checkbox",
                                        checked: is_selected,
                                        class: "rounded border-line text-brand focus:ring-brand",
                                        onchange: {
                                            let key = key.clone();
                                            move |_| {
                                                let mut stages = selected_stages.write();
                                                if let Some(pos) = stages.iter().position(|x| x == &key) {
                                                    stages.remove(pos);
                                                } else {
                                                    stages.push(key.clone());
                                                }
                                            }
                                        },
                                    }
                                    span { class: "font-semibold", {t!("rollout-form-all-clusters")} }
                                    if let Some(idx) = order {
                                        span { class: "text-xs text-fg-faint",
                                            {t!("rollout-form-stage-num", num: idx)}
                                        }
                                    }
                                }
                            }
                        }

                        // Regular groups
                        for g in &group_list_clone {
                            {
                                let gid = g.id.to_string();
                                let gname = g.name.clone();
                                let is_selected = selected_stages.read().contains(&gid);
                                let order = selected_stages
                                    .read()
                                    .iter()
                                    .position(|x| x == &gid);
                                rsx! {
                                    div { class: "flex items-center gap-2 mb-1",
                                        input {
                                            r#type: "checkbox",
                                            checked: is_selected,
                                            class: "rounded border-line text-brand focus:ring-brand",
                                            onchange: {
                                                let gid = gid.clone();
                                                move |_| {
                                                    let mut stages = selected_stages.write();
                                                    if let Some(pos) = stages.iter().position(|x| x == &gid) {
                                                        stages.remove(pos);
                                                    } else {
                                                        stages.push(gid.clone());
                                                    }
                                                }
                                            },
                                        }
                                        span { "{gname}" }
                                        if let Some(idx) = order {
                                            span { class: "text-xs text-fg-faint",
                                                {t!("rollout-form-stage-num", num: idx)}
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if group_list_clone.is_empty() {
                            p { class: "text-fg-muted text-xs mt-1",
                                Link { to: Route::RolloutGroupList {}, class: "link",
                                    {t!("rollout-form-create-groups-prefix")}
                                }
                                {t!("rollout-form-create-groups-suffix")}
                            }
                        }
                    }

                    // ── Health gate ──
                    div { class: "border-t border-line-soft pt-4",
                        div { class: "flex items-center gap-2 mb-2",
                            input {
                                r#type: "checkbox",
                                checked: gate.read().enabled,
                                class: "rounded border-line text-brand focus:ring-brand",
                                onchange: move |e| {
                                    gate.write().enabled = e.value() == "true";
                                },
                            }
                            label { class: "text-sm font-medium text-fg-strong",
                                {t!("rollout-form-health-gate")}
                            }
                            span { class: "text-xs text-fg-faint",
                                {t!("rollout-form-auto-pause")}
                            }
                        }

                        if gate.read().enabled {
                            div { class: "ml-6 space-y-3",
                                div { class: "grid grid-cols-1 sm:grid-cols-3 gap-3",
                                    div {
                                        label { class: "block text-xs font-medium text-fg mb-1",
                                            {t!("rollout-form-heartbeat-pct")}
                                        }
                                        input {
                                            r#type: "number",
                                            min: "0",
                                            max: "100",
                                            class: "input input-sm",
                                            value: "{gate.read().min_heartbeat_fresh_pct}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<u8>() {
                                                    gate.write().min_heartbeat_fresh_pct = v.min(100);
                                                }
                                            },
                                        }
                                    }
                                    div {
                                        label { class: "block text-xs font-medium text-fg mb-1",
                                            {t!("rollout-form-heartbeat-window")}
                                        }
                                        input {
                                            r#type: "number",
                                            min: "10",
                                            class: "input input-sm",
                                            value: "{gate.read().heartbeat_freshness_secs}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<u32>() {
                                                    gate.write().heartbeat_freshness_secs = v.max(10);
                                                }
                                            },
                                        }
                                    }
                                    div {
                                        label { class: "block text-xs font-medium text-fg mb-1",
                                            {t!("rollout-form-grace-period")}
                                        }
                                        input {
                                            r#type: "number",
                                            min: "0",
                                            class: "input input-sm",
                                            value: "{gate.read().grace_period_secs}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<u32>() {
                                                    gate.write().grace_period_secs = v;
                                                }
                                            },
                                        }
                                    }
                                }

                                div {
                                    label { class: "block text-xs font-medium text-fg mb-1",
                                        {t!("rollout-form-probe-thresholds")}
                                    }
                                    {
                                        let rows: Vec<(usize, String, u8)> = gate
                                            .read()
                                            .probe_thresholds
                                            .iter()
                                            .enumerate()
                                            .map(|(i, (s, p))| (i, s.clone(), *p))
                                            .collect();
                                        rsx! {
                                            for (idx, svc, pct) in rows {
                                                div { class: "flex items-center gap-2 mb-1",
                                                    input { class: "input input-sm flex-1 w-auto",
                                                        placeholder: t!("rollout-form-service-placeholder"),
                                                        value: "{svc}",
                                                        oninput: move |e| {
                                                            let mut g = gate.write();
                                                            if let Some(row) = g.probe_thresholds.get_mut(idx) {
                                                                row.0 = e.value();
                                                            }
                                                        },
                                                    }
                                                    input {
                                                        r#type: "number",
                                                        min: "0",
                                                        max: "100",
                                                        class: "input input-sm w-20",
                                                        value: "{pct}",
                                                        oninput: move |e| {
                                                            if let Ok(v) = e.value().parse::<u8>() {
                                                                let mut g = gate.write();
                                                                if let Some(row) = g.probe_thresholds.get_mut(idx) {
                                                                    row.1 = v.min(100);
                                                                }
                                                            }
                                                        },
                                                    }
                                                    span { class: "text-xs text-fg-muted", {t!("rollout-form-pct-symbol")} }
                                                    button { class: "link-danger text-xs",
                                                        onclick: move |_| {
                                                            let mut g = gate.write();
                                                            if idx < g.probe_thresholds.len() {
                                                                g.probe_thresholds.remove(idx);
                                                            }
                                                        },
                                                        {t!("rollout-form-remove-service")}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    button { class: "link text-xs mt-1",
                                        onclick: move |_| {
                                            gate.write().probe_thresholds.push((String::new(), 90));
                                        },
                                        {t!("rollout-form-add-service")}
                                    }
                                    HelpText { xs: true, class: "mt-1", {t!("rollout-form-gate-help")} }
                                }
                            }
                        }
                    }

                    if let Some(err) = &*error.read() {
                        ErrorText { "{err}" }
                    }

                    Button { size: ButtonSize::Lg,
                        onclick: move |_| {
                            let rollout_name = {
                                let n = name.read().trim().to_string();
                                if n.is_empty() { None } else { Some(n) }
                            };
                            let ver = {
                                let v = target_version.read().trim().to_string();
                                if v.is_empty() { None } else { Some(v) }
                            };
                            let stages = selected_stages.read().clone();
                            let commit = {
                                let c = nixpkgs_commit.read().trim().to_string();
                                if c.is_empty() { None } else { Some(c) }
                            };
                            let gate_input = gate.read().clone();
                            let ramp = {
                                let r = ramp_minutes.read().trim().to_string();
                                if r.is_empty() { Ok(None) } else { r.parse::<i32>().map(Some) }
                            };
                            async move {
                                if ver.is_none() && commit.is_none() {
                                    error.set(Some("Set at least one of target version or nixpkgs commit".into()));
                                    return;
                                }
                                if stages.is_empty() {
                                    error.set(Some("Select at least one stage".into()));
                                    return;
                                }
                                let ramp = match ramp {
                                    Ok(r) if r.is_none_or(|m| m > 0) => r,
                                    _ => {
                                        error.set(Some("Ramp duration must be a positive number of minutes".into()));
                                        return;
                                    }
                                };
                                let input = RolloutCreateInput {
                                    name: rollout_name,
                                    target_version: ver,
                                    stage_ids: stages,
                                    nixpkgs_commit: commit,
                                    gate: Some(gate_input.into()),
                                    ramp_minutes: ramp,
                                };
                                match create_rollout(input).await {
                                    Ok(id) => { nav.push(Route::RolloutDetail { id }); }
                                    Err(e) => error.set(Some(e.to_string())),
                                }
                            }
                        },
                        {t!("rollout-form-create")}
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
