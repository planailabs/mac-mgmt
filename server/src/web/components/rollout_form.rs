use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonSize, ErrorText, FormField, HelpText, PageHeader};
use crate::web::gate_input::HealthGateInput;
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

const ALL_CLUSTERS_SENTINEL: &str = "__all__";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupOption {
    id: Uuid,
    name: String,
}

#[server]
async fn get_available_versions() -> Result<Vec<String>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let versions = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT version FROM daemon_versions ORDER BY version DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(versions)
}

#[server]
async fn get_group_options() -> Result<Vec<GroupOption>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM rollout_groups \
         WHERE id != '00000000-0000-0000-0000-000000000000'::uuid \
         ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| GroupOption {
            id: r.id,
            name: r.name,
        })
        .collect())
}

/// `stage_ids` is an ordered list of group UUIDs or `"__all__"` sentinel.
/// `"__all__"` maps to the nil UUID (`00000000-…`) sentinel in `rollout_stages.group_id`;
/// queries that resolve stage members use a subquery for all clusters when
/// the sentinel is present instead of joining through `rollout_group_members`.
#[server]
async fn create_rollout(
    name: Option<String>,
    target_version: Option<String>,
    stage_ids: Vec<String>,
    nixpkgs_commit: Option<String>,
    gate: Option<HealthGateInput>,
    ramp_minutes: Option<i32>,
) -> Result<String, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let target_version = target_version.and_then(|v| {
        let t = v.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });
    if let Some(v) = &target_version {
        let parts: Vec<&str> = v.split('.').collect();
        let semver = parts.len() >= 3 && parts.iter().all(|p| p.parse::<u64>().is_ok());
        if !semver {
            // Non-semver channel versions ("rolling") are allowed when
            // they exist in daemon_versions (synced from xzar).
            let known: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM daemon_versions WHERE version = $1)",
            )
            .bind(v)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            if !known {
                return Err(ServerFnError::new(
                    "version must be semver (e.g., 0.1.6) or a known channel (e.g., rolling)",
                ));
            }
        }
    }

    if stage_ids.is_empty() {
        return Err(ServerFnError::new("select at least one stage"));
    }

    let nixpkgs_commit = nixpkgs_commit.and_then(|c| {
        let t = c.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });
    if let Some(c) = &nixpkgs_commit {
        let valid = (7..=40).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err(ServerFnError::new("nixpkgs commit must be 7-40 hex chars"));
        }
    }

    if target_version.is_none() && nixpkgs_commit.is_none() {
        return Err(ServerFnError::new(
            "set at least one of target version or nixpkgs commit",
        ));
    }

    if matches!(ramp_minutes, Some(m) if m <= 0) {
        return Err(ServerFnError::new("ramp duration must be positive"));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut resolved: Vec<Uuid> = Vec::with_capacity(stage_ids.len());
    for sid in &stage_ids {
        if sid == "__all__" {
            resolved.push(Uuid::nil());
        } else {
            let gid: Uuid = sid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            resolved.push(gid);
        }
    }

    if let Some(ver) = &target_version {
        #[derive(sqlx::FromRow)]
        struct DowngradeRow {
            cluster_name: String,
            pinned_version: String,
            group_name: String,
        }

        let downgrades = sqlx::query_as::<_, DowngradeRow>(
            "SELECT DISTINCT c.name AS cluster_name, c.pinned_version, rg.name AS group_name \
             FROM unnest($1::uuid[]) AS gid \
             JOIN rollout_groups rg ON rg.id = gid \
             JOIN LATERAL ( \
               SELECT cluster_id FROM rollout_group_members WHERE group_id = gid \
               UNION ALL \
               SELECT id FROM clusters WHERE gid = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             JOIN clusters c ON c.id = rgm.cluster_id \
             WHERE c.pinned_version IS NOT NULL \
               AND c.pinned_version > $2 \
             ORDER BY c.name, rg.name",
        )
        .bind(&resolved)
        .bind(ver)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if !downgrades.is_empty() {
            let mut by_cluster: std::collections::BTreeMap<String, (String, Vec<String>)> =
                std::collections::BTreeMap::new();
            for d in &downgrades {
                by_cluster
                    .entry(d.cluster_name.clone())
                    .or_insert_with(|| (d.pinned_version.clone(), Vec::new()))
                    .1
                    .push(d.group_name.clone());
            }
            let details: Vec<String> = by_cluster
                .into_iter()
                .map(|(name, (cur, groups))| format!("{name} (v{cur}, in: {})", groups.join(", ")))
                .collect();
            return Err(ServerFnError::new(format!(
                "Would downgrade to {ver}: {}",
                details.join("; ")
            )));
        }
    }

    if let Some(nix) = &nixpkgs_commit {
        let current_commits: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT c.nixpkgs_commit \
             FROM unnest($1::uuid[]) AS gid \
             JOIN LATERAL ( \
               SELECT cluster_id FROM rollout_group_members WHERE group_id = gid \
               UNION ALL \
               SELECT id FROM clusters WHERE gid = '00000000-0000-0000-0000-000000000000'::uuid \
             ) rgm ON true \
             JOIN clusters c ON c.id = rgm.cluster_id \
             WHERE c.nixpkgs_commit IS NOT NULL",
        )
        .bind(&resolved)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        let mut all_shas: std::collections::HashSet<String> = [nix.clone()].into_iter().collect();
        all_shas.extend(current_commits.iter().cloned());
        let counts = crate::commit_count::nixpkgs_commit_counts(&all_shas).await;

        let new_count = counts
            .get(nix)
            .ok_or_else(|| ServerFnError::new(format!("unknown nixpkgs commit {nix}")))?;
        for cur in &current_commits {
            let cur_count = counts.get(cur).ok_or_else(|| {
                ServerFnError::new(format!("cannot resolve commit count for current {cur}"))
            })?;
            if new_count < cur_count {
                let short_new: String = nix.chars().take(12).collect();
                let short_cur: String = cur.chars().take(12).collect();
                return Err(ServerFnError::new(format!(
                    "nixpkgs {short_new} (#{new_count}) is older than current {short_cur} (#{cur_count}); use rollback to downgrade"
                )));
            }
        }
    }

    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let rollout_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rollouts (id, name, target_version, nixpkgs_commit) VALUES ($1, $2, $3, $4)",
    )
    .bind(rollout_id)
    .bind(&name)
    .bind(&target_version)
    .bind(&nixpkgs_commit)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let gate_json: Option<serde_json::Value> = gate.as_ref().and_then(|g| g.to_json());

    for (i, gid) in resolved.iter().enumerate() {
        sqlx::query(
            "INSERT INTO rollout_stages (rollout_id, group_id, stage_order, health_gate, ramp_minutes) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(rollout_id)
        .bind(gid)
        .bind(i as i32)
        .bind(&gate_json)
        .bind(ramp_minutes)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rollout_id.to_string())
}

#[component]
pub fn RolloutForm() -> Element {
    use_topbar(t!("nav-rollouts"), None);
    let groups = use_server_future(move || async move { get_group_options().await })?;
    let versions = use_server_future(move || async move { get_available_versions().await })?;
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
                                match create_rollout(rollout_name, ver, stages, commit, Some(gate_input), ramp).await {
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
