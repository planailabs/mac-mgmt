use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

const ALL_CLUSTERS_SENTINEL: &str = "__all__";

/// Form-side mirror of `rollout_health::HealthGate`. Kept verbatim so the
/// JSON we POST is round-trip compatible with the evaluator's struct.
/// `min_probe_ok_pct` is shipped as a `Vec<(String, u8)>` rather than a
/// HashMap so the UI can render insertion-ordered rows; the server
/// converts back to a HashMap at write time.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthGateInput {
    enabled: bool,
    min_heartbeat_fresh_pct: u8,
    heartbeat_freshness_secs: u32,
    grace_period_secs: u32,
    /// Each entry is (service_name, required_ok_pct). Empty string keys
    /// are dropped server-side.
    probe_thresholds: Vec<(String, u8)>,
}

impl Default for HealthGateInput {
    fn default() -> Self {
        Self {
            enabled: true,
            min_heartbeat_fresh_pct: 95,
            heartbeat_freshness_secs: 180,
            grace_period_secs: 600,
            probe_thresholds: vec![
                ("openclaw".into(), 90),
                ("ollama".into(), 90),
            ],
        }
    }
}

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
    target_version: Option<String>,
    stage_ids: Vec<String>,
    nixpkgs_commit: Option<String>,
    gate: Option<HealthGateInput>,
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
        if parts.len() < 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
            return Err(ServerFnError::new("version must be semver (e.g., 0.1.6)"));
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
            return Err(ServerFnError::new(
                "nixpkgs commit must be 7-40 hex chars",
            ));
        }
    }

    if target_version.is_none() && nixpkgs_commit.is_none() {
        return Err(ServerFnError::new(
            "set at least one of target version or nixpkgs commit",
        ));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Resolve each stage_id to a real group UUID.
    // "__all__" maps to the nil-UUID sentinel (no temp group created).
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

    // Downgrade check: only meaningful when a target_version is set.
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
            // Group by cluster
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

    let rollout_id = Uuid::new_v4();
    sqlx::query("INSERT INTO rollouts (id, target_version, nixpkgs_commit) VALUES ($1, $2, $3)")
        .bind(rollout_id)
        .bind(&target_version)
        .bind(&nixpkgs_commit)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Build the JSON gate payload once. Empty service rows are dropped
    // and percentages clamped so an off-by-one doesn't poison evaluation.
    let gate_json: Option<serde_json::Value> = match gate {
        Some(g) if g.enabled => {
            let mut probe_map = serde_json::Map::new();
            for (svc, pct) in &g.probe_thresholds {
                let svc = svc.trim();
                if svc.is_empty() {
                    continue;
                }
                probe_map.insert(
                    svc.to_string(),
                    serde_json::Value::Number((*pct).min(100).into()),
                );
            }
            Some(serde_json::json!({
                "min_heartbeat_fresh_pct": g.min_heartbeat_fresh_pct.min(100),
                "heartbeat_freshness_secs": g.heartbeat_freshness_secs.max(10),
                "min_probe_ok_pct": serde_json::Value::Object(probe_map),
                "grace_period_secs": g.grace_period_secs,
            }))
        }
        _ => None,
    };

    for (i, gid) in resolved.iter().enumerate() {
        sqlx::query(
            "INSERT INTO rollout_stages (rollout_id, group_id, stage_order, health_gate) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(rollout_id)
        .bind(gid)
        .bind(i as i32)
        .bind(&gate_json)
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
    let groups = use_server_future(move || async move { get_group_options().await })?;
    let versions = use_server_future(move || async move { get_available_versions().await })?;
    let mut target_version = use_signal(String::new);
    let mut nixpkgs_commit = use_signal(String::new);
    let mut selected_stages = use_signal(Vec::<String>::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut gate = use_signal(HealthGateInput::default);
    let nav = navigator();

    match &*groups.read() {
        Some(Ok(group_list)) => {
            let group_list_clone = group_list.clone();
            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    h2 { class: "text-2xl font-bold", "New Version Rollout" }
                    Link {
                        to: Route::RolloutGroupList {},
                        class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                        "Manage Groups"
                    }
                }

                div { class: "space-y-4",
                    div {
                        label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1",
                            "Target Version"
                        }
                        {
                            let version_list: Vec<String> = match &*versions.read() {
                                Some(Ok(v)) => v.clone(),
                                _ => Vec::new(),
                            };
                            rsx! {
                                select {
                                    class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 text-sm font-mono dark:bg-gray-700 dark:text-white",
                                    value: "{target_version}",
                                    onchange: move |e| target_version.set(e.value()),
                                    option { value: "", "— none (nixpkgs only) —" }
                                    for v in version_list.iter() {
                                        option { value: "{v}", "{v}" }
                                    }
                                }
                                if version_list.is_empty() {
                                    p { class: "text-xs text-amber-600 dark:text-amber-500 mt-1",
                                        "No daemon versions available. Sync them on the Daemon Versions page."
                                    }
                                }
                            }
                        }
                        p { class: "text-xs text-gray-400 dark:text-gray-500 mt-1",
                            "Pick a version uploaded via xzar. Downgrades are blocked."
                        }
                    }
                    div {
                        label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1",
                            "Nixpkgs Commit (optional)"
                        }
                        input {
                            class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 text-sm font-mono dark:bg-gray-700 dark:text-white",
                            placeholder: "e.g. 170a4b510ad7ee95dde01adf2fe21704498dbb5c",
                            value: "{nixpkgs_commit}",
                            oninput: move |e| nixpkgs_commit.set(e.value()),
                        }
                        p { class: "text-xs text-gray-400 dark:text-gray-500 mt-1",
                            "Pin the nixpkgs source to this commit. Leave blank to leave each cluster's existing pin untouched."
                        }
                    }
                    div {
                        label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1",
                            "Stages (select in order)"
                        }

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
                                    span { class: "font-semibold", "All Clusters" }
                                    if let Some(idx) = order {
                                        span { class: "text-xs text-gray-400 dark:text-gray-500",
                                            "(stage {idx})"
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
                                            onchange: {
                                                let gid = gid.clone();
                                                move |_| {
                                                    let mut stages =
                                                        selected_stages.write();
                                                    if let Some(pos) =
                                                        stages.iter().position(|x| x == &gid)
                                                    {
                                                        stages.remove(pos);
                                                    } else {
                                                        stages.push(gid.clone());
                                                    }
                                                }
                                            },
                                        }
                                        span { "{gname}" }
                                        if let Some(idx) = order {
                                            span { class: "text-xs text-gray-400 dark:text-gray-500",
                                                "(stage {idx})"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if group_list_clone.is_empty() {
                            p { class: "text-gray-500 dark:text-gray-400 text-xs mt-1",
                                Link {
                                    to: Route::RolloutGroupList {},
                                    class: "text-blue-600 dark:text-blue-400 hover:underline",
                                    "Create groups"
                                }
                                " to roll out in stages."
                            }
                        }
                    }

                    // ── Health gate ──
                    div { class: "border-t border-gray-200 dark:border-gray-700 pt-4",
                        div { class: "flex items-center gap-2 mb-2",
                            input {
                                r#type: "checkbox",
                                checked: gate.read().enabled,
                                onchange: move |e| {
                                    gate.write().enabled = e.value() == "true";
                                },
                            }
                            label { class: "text-sm font-medium text-gray-700 dark:text-gray-200",
                                "Health gate"
                            }
                            span { class: "text-xs text-gray-400 dark:text-gray-500",
                                "Auto-pause stages when assessment data falls below thresholds"
                            }
                        }

                        if gate.read().enabled {
                            div { class: "ml-6 space-y-3",
                                div { class: "grid grid-cols-1 sm:grid-cols-3 gap-3",
                                    div {
                                        label { class: "block text-xs font-medium text-gray-600 dark:text-gray-300 mb-1",
                                            "Heartbeat fresh % (min)"
                                        }
                                        input {
                                            r#type: "number",
                                            min: "0",
                                            max: "100",
                                            class: "w-full border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white",
                                            value: "{gate.read().min_heartbeat_fresh_pct}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<u8>() {
                                                    gate.write().min_heartbeat_fresh_pct = v.min(100);
                                                }
                                            },
                                        }
                                    }
                                    div {
                                        label { class: "block text-xs font-medium text-gray-600 dark:text-gray-300 mb-1",
                                            "Heartbeat freshness window (sec)"
                                        }
                                        input {
                                            r#type: "number",
                                            min: "10",
                                            class: "w-full border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white",
                                            value: "{gate.read().heartbeat_freshness_secs}",
                                            oninput: move |e| {
                                                if let Ok(v) = e.value().parse::<u32>() {
                                                    gate.write().heartbeat_freshness_secs = v.max(10);
                                                }
                                            },
                                        }
                                    }
                                    div {
                                        label { class: "block text-xs font-medium text-gray-600 dark:text-gray-300 mb-1",
                                            "Grace period after start (sec)"
                                        }
                                        input {
                                            r#type: "number",
                                            min: "0",
                                            class: "w-full border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white",
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
                                    label { class: "block text-xs font-medium text-gray-600 dark:text-gray-300 mb-1",
                                        "Probe success thresholds"
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
                                                    input {
                                                        class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white flex-1",
                                                        placeholder: "service (e.g. ollama)",
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
                                                        class: "border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm w-20 dark:bg-gray-700 dark:text-white",
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
                                                    span { class: "text-xs text-gray-500 dark:text-gray-400", "%" }
                                                    button {
                                                        class: "text-red-600 dark:text-red-400 text-xs hover:underline",
                                                        onclick: move |_| {
                                                            let mut g = gate.write();
                                                            if idx < g.probe_thresholds.len() {
                                                                g.probe_thresholds.remove(idx);
                                                            }
                                                        },
                                                        "remove"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    button {
                                        class: "text-blue-600 dark:text-blue-400 text-xs hover:underline mt-1",
                                        onclick: move |_| {
                                            gate.write().probe_thresholds.push((String::new(), 90));
                                        },
                                        "+ add service"
                                    }
                                    p { class: "text-xs text-gray-400 dark:text-gray-500 mt-1",
                                        "A stage fails its gate if any listed service drops below the threshold over the last 30 min."
                                    }
                                }
                            }
                        }
                    }

                    if let Some(err) = &*error.read() {
                        p { class: "text-red-600 dark:text-red-400 text-sm", "{err}" }
                    }

                    button {
                        class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                        onclick: move |_| {
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
                            async move {
                                if ver.is_none() && commit.is_none() {
                                    error.set(Some("Set at least one of target version or nixpkgs commit".into()));
                                    return;
                                }
                                if stages.is_empty() {
                                    error.set(Some("Select at least one stage".into()));
                                    return;
                                }
                                match create_rollout(ver, stages, commit, Some(gate_input)).await {
                                    Ok(id) => {
                                        nav.push(Route::RolloutDetail { id });
                                    }
                                    Err(e) => error.set(Some(e.to_string())),
                                }
                            }
                        },
                        "Create Rollout"
                    }
                }
            }
        }
        Some(Err(e)) => rsx! {
            p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" }
        },
        None => rsx! {
            p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." }
        },
    }
}
