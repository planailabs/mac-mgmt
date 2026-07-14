use dioxus::prelude::*;
use dioxus_i18n::t;
use uuid::Uuid;

use crate::api_mcp::endpoints::rollouts::{
    ProbeStatsView, RolloutActionInput, RolloutGetInput, RolloutHealthSummary, StageGateGetInput,
    StageGateUpdateInput, StagePushInput, StageReevaluateInput, get_rollout_detail, get_stage_gate,
    reevaluate_stage, request_stage_assessment, rollout_action, trigger_stage_self_update,
    trigger_stage_sync_nixpkgs, update_stage_gate,
};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, ErrorText, HelpText, Kicker,
    Mono, Pill, PillVariant, SectionHeading, StageItem, StageStatus, StageTimeline,
};
use crate::web::gate_input::HealthGateInput;

fn status_variant(status: &str) -> BadgeVariant {
    match status {
        "rolling" => BadgeVariant::Info,
        "completed" => BadgeVariant::Success,
        "paused" | "rolled_back" => BadgeVariant::Warn,
        "failed" => BadgeVariant::Danger,
        _ => BadgeVariant::Neutral,
    }
}

fn health_state_variant(state: &str) -> BadgeVariant {
    match state {
        "pass" => BadgeVariant::Success,
        "fail" => BadgeVariant::Danger,
        "grace" => BadgeVariant::Warn,
        _ => BadgeVariant::Neutral,
    }
}

#[server]
async fn get_nixpkgs_commit_count_rollout(sha: String) -> Result<Option<u64>, ServerFnError> {
    let shas = std::collections::HashSet::from([sha.clone()]);
    let counts = crate::commit_count::nixpkgs_commit_counts(&shas).await;
    Ok(counts.get(&sha).copied())
}

/// Render the rollout-wide health summary as a card above the action
/// buttons. Mirrors the shape used in the rollout-list Health column but
/// with extra detail (heartbeat-freshness avg, per-service probe %)
/// surfaced as inline metrics.
fn render_health_summary_card(hs: &RolloutHealthSummary) -> Element {
    let variant = health_state_variant(&hs.state);
    let label = match hs.state.as_str() {
        "pass" => t!("rollout-health-pass"),
        "fail" => t!("rollout-health-fail"),
        "grace" => t!("rollout-health-grace"),
        _ => t!("rollout-health-no-data"),
    };
    let hb = hs
        .avg_heartbeat_fresh_pct
        .map(|v| format!("{v}%"))
        .unwrap_or_else(|| t!("em-dash"));

    rsx! {
        div { class: "mb-6 p-4 card",
            div { class: "flex items-center gap-3 mb-2",
                span { class: "text-lg font-semibold text-fg-strong",
                    {t!("rollout-detail-health")}
                }
                Badge { variant, "{label}" }
                if hs.failing_stages > 0 {
                    span { class: "text-xs font-mono text-danger-strong",
                        {t!("rollout-detail-stages-failing", failing: hs.failing_stages, total: hs.evaluated_stages)}
                    }
                } else if hs.evaluated_stages > 0 {
                    span { class: "text-xs font-mono text-fg-muted",
                        {t!("rollout-detail-stages-passing", evaluated: hs.evaluated_stages, total: hs.evaluated_stages)}
                    }
                }
            }
            div { class: "flex flex-wrap gap-4 text-xs text-fg",
                span {
                    span { class: "font-medium", {t!("rollout-detail-cohort")} }
                    "{hs.total_cohort}"
                }
                span { class: "font-mono",
                    span { class: "font-medium font-sans", {t!("rollout-detail-heartbeats")} }
                    "{hb}"
                }
                {
                    let mut probe_pairs: Vec<(&String, &u8)> = hs.probe_ok_pct.iter().collect();
                    probe_pairs.sort_by(|a, b| a.0.cmp(b.0));
                    rsx! {
                        for (svc, pct) in probe_pairs {
                            span { class: "font-mono",
                                span { class: "font-medium font-sans", {t!("rollout-detail-service-probe", service: svc.clone())} }
                                "{pct}%"
                            }
                        }
                    }
                }
            }
            if !hs.top_reason.is_empty() {
                p { class: "mt-2 text-xs text-danger-strong",
                    {t!("rollout-detail-top-reason", reason: hs.top_reason.clone())}
                }
            }
        }
    }
}

/// Block on a browser `confirm()` dialog. Returns `true` on OK, `false`
/// on Cancel or any JS hiccup. Used to gate destructive rollout actions
/// (rollback, complete, delete) so a misclick doesn't instantly rewind
/// cluster pins or remove historical rows.
async fn confirm_prompt(message: &str) -> bool {
    // Escape single quotes so the message doesn't break the JS literal.
    // Browsers treat confirm() as synchronous; the outer dioxus.send
    // ships the bool back to Rust via the channel recv below.
    let script = format!(
        "dioxus.send(confirm('{}'))",
        message.replace('\\', "\\\\").replace('\'', "\\'")
    );
    document::eval(&script)
        .recv::<serde_json::Value>()
        .await
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[component]
pub fn RolloutDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut detail = use_server_future(move || {
        let id = id_clone.clone();
        async move {
            let id: Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_rollout_detail(RolloutGetInput { id }).await
        }
    })?;

    // Topbar shows the human-friendly rollout name (or short id when
    // none was given). The "Rollouts" subtitle keeps it grounded in the
    // section navigation.
    let topbar_title = match &*detail.read() {
        Some(Ok(info)) => info
            .name
            .clone()
            .unwrap_or_else(|| format!("Rollout {}", &info.id.to_string()[..8])),
        _ => String::new(),
    };
    use_topbar(topbar_title, Some(t!("nav-rollouts").to_string()));
    let nav = navigator();
    // Inline feedback for "Request fresh assessment" — Option<(stage_id,
    // message, is_error)>. Cleared when the operator clicks a different
    // stage's button. Avoids the silent click that prompted this work.
    let mut request_status = use_signal(|| Option::<(String, String, bool)>::None);
    // Stage_id of the currently in-flight Reevaluate call. Rapid clicks on
    // the same button used to stack up concurrent detail.restart()s, each
    // of which flashed the page back to Loading for a moment. Gating the
    // click handler on this signal turns spam into a no-op.
    let mut reevaluating_stage = use_signal(|| Option::<String>::None);
    // Per-stage gate editor: Option<(stage_id, HealthGateInput, apply_to_all)>
    // is None when no editor is open. Only one stage edits at a time — opening
    // another closes the first without prompting (the form has Cancel anyway).
    let mut edit_gate = use_signal(|| Option::<(String, HealthGateInput, bool)>::None);
    let mut edit_gate_error = use_signal(|| Option::<String>::None);

    match &*detail.read() {
        Some(Ok(info)) => {
            let rid = info.id.to_string();
            let rid_id = info.id;
            let status = info.status.clone();
            let created = info.created_at.format("%Y-%m-%d %H:%M").to_string();
            let has_version_update = info.target_version.is_some();
            let has_nixpkgs_update = info.nixpkgs_commit.is_some();

            let badge_variant = status_variant(&info.status);

            let display_name = info
                .name
                .clone()
                .unwrap_or_else(|| t!("rollout-detail-rollout-prefix", id: &rid[..8]));
            // ── Page hero ─────────────────────────────────────────
            // Matches the design's Rollouts hero: status pill + created
            // timestamp on top, monospace display title underneath, and
            // the action buttons on the right (Rollback / Delete come
            // through unchanged below).
            let status_pill_variant = match info.status.as_str() {
                "rolling" => PillVariant::Accent,
                "completed" => PillVariant::Ok,
                "paused" | "rolled_back" => PillVariant::Warn,
                "failed" => PillVariant::Bad,
                _ => PillVariant::Muted,
            };
            // The id is a UUID; rendering the full string is noise. Show
            // the first 8 chars in mono — same convention as the design.
            let short_id: String = rid.chars().take(8).collect();
            let _ = badge_variant; // suppress unused-warning for legacy pill
            rsx! {
                div { class: "flex flex-col xl:flex-row xl:justify-between xl:items-end gap-3 mb-5",
                    div { class: "min-w-0",
                        div { class: "flex items-center gap-2 mb-2 flex-wrap",
                            Pill { variant: status_pill_variant, "{status}" }
                            span { class: "text-fg-muted text-xs font-mono",
                                {t!("rollout-detail-created-label", date: created.clone())}
                            }
                        }
                        Kicker { class: "mb-1", {t!("nav-rollouts")} }
                        h1 { class: "h-page mb-0 font-mono",
                            "{display_name}"
                            if info.name.is_some() {
                                span { class: "text-fg-muted ml-2 text-xl", "#{short_id}" }
                            }
                        }
                        div { class: "mt-2 flex items-center gap-2",
                            Mono { class: "text-xs text-fg-muted", "{rid}" }
                        }
                    }
                    div { class: "flex gap-2",
                        if matches!(info.status.as_str(), "rolling" | "paused" | "completed") {
                            Button { variant: ButtonVariant::Warn, size: ButtonSize::Sm,
                                title: "Mark rollout as rolled-back and rewind cluster pins to the baseline captured at start time",
                                onclick: {
                                    move |_| {
                                        let msg = t!("rollout-detail-rollback-confirm");
                                        async move {
                                            let ok = confirm_prompt(&msg).await;
                                            if !ok { return; }
                                            let _ = rollout_action(RolloutActionInput {
                                                id: rid_id,
                                                action: "rollback".into(),
                                            })
                                            .await;
                                            detail.restart();
                                        }
                                    }
                                },
                                {t!("rollout-detail-rollback")}
                            }
                        }
                        if matches!(info.status.as_str(), "pending" | "completed" | "failed" | "rolled_back") {
                            Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                onclick: {
                                    move |_| {
                                        let msg = t!("rollout-detail-delete-confirm");
                                        async move {
                                            let ok = confirm_prompt(&msg).await;
                                            if !ok { return; }
                                            let _ = rollout_action(RolloutActionInput {
                                                id: rid_id,
                                                action: "delete".into(),
                                            })
                                            .await;
                                            nav.push(Route::RolloutList {});
                                        }
                                    }
                                },
                                {t!("delete")}
                            }
                        }
                    }
                }

                // Rollout-wide health summary — visible only for rolling
                // rollouts with at least one gated stage. Aggregates the
                // most recent stored evaluation per stage (auto-pause loop
                // refreshes these every 60s).
                if let Some(hs) = info.health_summary.as_ref() {
                    {render_health_summary_card(hs)}
                }

                // Action buttons
                div { class: "flex gap-2 mb-6",
                    if info.status == "pending" {
                        Button { size: ButtonSize::Sm,
                            onclick: {
                                move |_| {
                                    async move {
                                        let _ = rollout_action(RolloutActionInput {
                                            id: rid_id,
                                            action: "start".into(),
                                        })
                                        .await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-start")}
                        }
                    }
                    if info.status == "rolling" {
                        Button { size: ButtonSize::Sm,
                            onclick: {
                                move |_| {
                                    async move {
                                        let _ = rollout_action(RolloutActionInput {
                                            id: rid_id,
                                            action: "advance".into(),
                                        })
                                        .await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-advance")}
                        }
                        Button { variant: ButtonVariant::Warn, size: ButtonSize::Sm,
                            onclick: {
                                move |_| {
                                    async move {
                                        let _ = rollout_action(RolloutActionInput {
                                            id: rid_id,
                                            action: "pause".into(),
                                        })
                                        .await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-pause")}
                        }
                    }
                    if info.status == "paused" {
                        Button { size: ButtonSize::Sm,
                            onclick: {
                                move |_| {
                                    async move {
                                        let _ = rollout_action(RolloutActionInput {
                                            id: rid_id,
                                            action: "resume".into(),
                                        })
                                        .await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-resume")}
                        }
                    }
                    if info.status != "completed" {
                        Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm,
                            onclick: {
                                move |_| {
                                    async move {
                                        let msg = t!("rollout-detail-complete-confirm");
                                        let ok = confirm_prompt(&msg).await;
                                        if !ok { return; }
                                        let _ = rollout_action(RolloutActionInput {
                                            id: rid_id,
                                            action: "complete".into(),
                                        })
                                        .await;
                                        detail.restart();
                                    }
                                }
                            },
                            {t!("rollout-detail-complete-all")}
                        }
                    }
                }

                // ── Stage timeline (design-language summary) ─────────
                // Horizontal connector with one circle per stage, the
                // way the design's Rollouts mock surfaces progress at a
                // glance. The detailed admin cards (gate config,
                // evaluations, gate edits) live below, since the
                // timeline only carries status + fleet rollup.
                {
                    let timeline_stages: Vec<StageItem> = info.stages.iter().map(|s| {
                        let status = match s.status.as_str() {
                            "completed" => StageStatus::Completed,
                            "rolling"   => StageStatus::InProgress,
                            _           => StageStatus::Pending,
                        };
                        let fleet_total  = s.total_count.max(0) as usize;
                        // The "fleet" filled tracks version-upgrade progress,
                        // matching the design's bars-fill-as-rollout-promotes
                        // intuition. Online-but-not-yet-upgraded reads as
                        // pending, which is the right colour weight.
                        let fleet_filled = s.upgraded_count.clamp(0, s.total_count) as usize;
                        let summary = if s.total_count > 0 {
                            format!(
                                "{}/{} online · nixpkgs {}/{}",
                                s.healthy_count, s.total_count,
                                s.nixpkgs_upgraded_count, s.total_count,
                            )
                        } else {
                            t!("rollout-detail-no-heartbeats").to_string()
                        };
                        StageItem {
                            name: s.group_name.clone(),
                            status,
                            fleet_total,
                            fleet_filled,
                            summary,
                        }
                    }).collect();
                    rsx! {
                        div { class: "mb-6",
                            StageTimeline { stages: timeline_stages }
                        }
                    }
                }

                // Stages — detailed admin view (per-stage gate config,
                // evaluations, gate editing). Kept as the source of
                // truth for actions; the timeline above is read-only.
                SectionHeading { {t!("rollout-detail-stages")} }
                div { class: "space-y-3 mb-6",
                    for stage in &info.stages {
                        {
                            let stage_badge_variant = status_variant(&stage.status);
                            let started = stage
                                .started_at
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let completed = stage
                                .completed_at
                                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let health_text = if stage.total_count > 0 {
                                t!("rollout-detail-online", healthy: stage.healthy_count, total: stage.total_count)
                            } else {
                                t!("rollout-detail-no-heartbeats")
                            };
                            let health_color = if stage.total_count == 0 {
                                "text-fg-faint"
                            } else if stage.healthy_count == stage.total_count {
                                "text-success"
                            } else {
                                "text-danger"
                            };
                            let version_upgrade_text = if stage.total_count > 0 {
                                t!("rollout-detail-version-progress", upgraded: stage.upgraded_count, total: stage.total_count)
                            } else {
                                String::new()
                            };
                            let version_upgrade_color = if stage.total_count == 0 {
                                "text-fg-faint"
                            } else if stage.upgraded_count == stage.total_count {
                                "text-success"
                            } else if stage.upgraded_count > 0 {
                                "text-info"
                            } else {
                                "text-fg-faint"
                            };
                            let nixpkgs_upgrade_text = if stage.total_count > 0 {
                                t!("rollout-detail-nixpkgs-progress", upgraded: stage.nixpkgs_upgraded_count, total: stage.total_count)
                            } else {
                                String::new()
                            };
                            let nixpkgs_upgrade_color = if stage.total_count == 0 {
                                "text-fg-faint"
                            } else if stage.nixpkgs_upgraded_count == stage.total_count {
                                "text-success"
                            } else if stage.nixpkgs_upgraded_count > 0 {
                                "text-info"
                            } else {
                                "text-fg-faint"
                            };

                            let stage_id_str = stage.id.to_string();
                            let stage_id_uuid = stage.id;

                            rsx! {
                                Card { class: "p-4",
                                    div { class: "flex justify-between items-center mb-2",
                                        div { class: "flex items-center gap-2",
                                            span { class: "font-medium text-sm",
                                                {t!("rollout-detail-stage-num", num: stage.stage_order)}
                                            }
                                            span { class: "text-fg", "{stage.group_name}" }
                                            Badge { variant: stage_badge_variant, "{stage.status}" }
                                        }
                                        div { class: "flex gap-3",
                                            if !version_upgrade_text.is_empty() {
                                                span { class: "text-sm font-medium {version_upgrade_color}",
                                                    "{version_upgrade_text}"
                                                }
                                            }
                                            if !nixpkgs_upgrade_text.is_empty() {
                                                span { class: "text-sm font-medium {nixpkgs_upgrade_color}",
                                                    "{nixpkgs_upgrade_text}"
                                                }
                                            }
                                            span { class: "text-sm font-medium {health_color}",
                                                "{health_text}"
                                            }
                                        }
                                    }
                                    div { class: "text-xs text-fg-faint flex gap-4",
                                        span { {t!("rollout-detail-started", date: started.clone())} }
                                        span { {t!("rollout-detail-completed", date: completed.clone())} }
                                        if let (Some(pct), Some(mins)) = (stage.ramp_pct, stage.ramp_minutes) {
                                            span { class: if pct < 100 { "text-info" } else { "" },
                                                {t!("rollout-detail-ramp", pct: pct, mins: mins)}
                                            }
                                        }
                                    }

                                    // No-gate affordance: one-line row with an
                                    // {t!("rollout-detail-add-gate")} button. Mirrors where the gate
                                    // panel would have sat, so the card layout
                                    // stays consistent across stages.
                                    if !stage.has_gate {
                                        div { class: "mt-3 pt-3 border-t border-line-soft flex items-center justify-between",
                                            span { class: "text-xs text-fg-muted",
                                                {t!("rollout-detail-no-gate")}
                                            }
                                            div { class: "flex gap-2",
                                                Link {
                                                    to: Route::FleetDashboard { stage_id: Some(stage_id_str.clone()) },
                                                    class: "btn btn-xs btn-secondary",
                                                    title: "Open the fleet dashboard filtered to this stage's cohort",
                                                    {t!("rollout-detail-view-fleet")}
                                                }
                                                button { class: "btn btn-xs btn-info-soft",
                                                    onclick: {
                                                        let sid = stage_id_str.clone();
                                                        move |_| {
                                                            let sid = sid.clone();
                                                            edit_gate_error.set(None);
                                                            edit_gate.set(Some((sid, HealthGateInput::default(), false)));
                                                        }
                                                    },
                                                    {t!("rollout-detail-add-gate")}
                                                }
                                            }
                                        }
                                    }

                                    // ── System-assessment health gate ──
                                    if stage.has_gate {
                                        {
                                            let evaluated_at_text = stage.health.as_ref().map(|h| h.evaluated_at.format("%Y-%m-%d %H:%M:%S").to_string());
                                            // "grace" is reserved for the case where the gate would have
                                            // failed if not for the grace period — i.e. there are reasons
                                            // but they're being shielded. A clean pass during grace renders
                                            // as a normal pass, since there's nothing for the grace period
                                            // to actually shield.
                                            let (gate_variant, gate_text) = match stage.health.as_ref() {
                                                Some(h) if h.passed && h.in_grace_period && !h.reasons.is_empty() =>
                                                    (BadgeVariant::Warn, t!("rollout-detail-gate-grace")),
                                                Some(h) if h.passed =>
                                                    (BadgeVariant::Success, t!("rollout-detail-gate-pass")),
                                                Some(_) =>
                                                    (BadgeVariant::Danger, t!("rollout-detail-gate-fail")),
                                                None =>
                                                    (BadgeVariant::Neutral, t!("rollout-detail-gate-no-data")),
                                            };
                                            rsx! {
                                                div { class: "mt-3 pt-3 border-t border-line-soft",
                                                    div { class: "flex justify-between items-center mb-2",
                                                        div { class: "flex items-center gap-2",
                                                            span { class: "text-sm font-semibold text-fg-strong",
                                                                {t!("rollout-detail-assessment-gate")}
                                                            }
                                                            Badge { variant: gate_variant, "{gate_text}" }
                                                            if let Some(ts) = evaluated_at_text.as_ref() {
                                                                span { class: "text-xs text-fg-muted",
                                                                    {t!("rollout-detail-evaluated", ts: ts.clone())}
                                                                }
                                                            }
                                                        }
                                                        div { class: "flex gap-2",
                                                            button { class: "btn btn-xs btn-secondary",
                                                                title: "Change thresholds, add/remove probed services, or disable the gate entirely",
                                                                onclick: {
                                                                    let sid = stage_id_str.clone();
                                                                    move |_| {
                                                                        let sid_inner = sid.clone();
                                                                        async move {
                                                                            edit_gate_error.set(None);
                                                                            match get_stage_gate(StageGateGetInput { stage_id: stage_id_uuid }).await {
                                                                                Ok(Some(existing)) => {
                                                                                    edit_gate.set(Some((sid_inner, existing.into(), false)));
                                                                                }
                                                                                Ok(None) => {
                                                                                    edit_gate.set(Some((sid_inner, HealthGateInput::default(), false)));
                                                                                }
                                                                                Err(e) => {
                                                                                    edit_gate_error.set(Some(e.to_string()));
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                },
                                                                {t!("rollout-detail-edit-gate")}
                                                            }
                                                            {
                                                                let busy = reevaluating_stage
                                                                    .read()
                                                                    .as_ref()
                                                                    .map(|s| s == &stage_id_str)
                                                                    .unwrap_or(false);
                                                                let label = if busy { t!("rollout-detail-reevaluating") } else { t!("rollout-detail-reevaluate-now") };
                                                                rsx! {
                                                                    button { class: "btn btn-xs btn-secondary",
                                                                        disabled: busy,
                                                                        onclick: {
                                                                            let sid = stage_id_str.clone();
                                                                            move |_| {
                                                                                let sid = sid.clone();
                                                                                async move {
                                                                                    // Gate so spam turns into a no-op rather
                                                                                    // than stacking up detail.restart()s, each
                                                                                    // of which flashes Loading.
                                                                                    if reevaluating_stage.read().as_ref() == Some(&sid) {
                                                                                        return;
                                                                                    }
                                                                                    reevaluating_stage.set(Some(sid));
                                                                                    let _ = reevaluate_stage(StageReevaluateInput {
                                                                                        stage_id: stage_id_uuid,
                                                                                    })
                                                                                    .await;
                                                                                    reevaluating_stage.set(None);
                                                                                    detail.restart();
                                                                                }
                                                                            }
                                                                        },
                                                                        "{label}"
                                                                    }
                                                                }
                                                            }
                                                            button { class: "btn btn-xs btn-info-soft",
                                                                onclick: {
                                                                    let sid = stage_id_str.clone();
                                                                    move |_| {
                                                                        let sid = sid.clone();
                                                                        request_status.set(Some((sid.clone(), t!("rollout-detail-requesting"), false)));
                                                                        async move {
                                                                            match request_stage_assessment(StagePushInput { stage_id: stage_id_uuid }).await {
                                                                                Ok(r) => {
                                                                                    let msg = if r.dispatched == 0 {
                                                                                        if r.cohort_size == 0 {
                                                                                            t!("rollout-detail-no-clusters")
                                                                                        } else {
                                                                                            t!("rollout-detail-no-daemons", cohort: r.cohort_size)
                                                                                        }
                                                                                    } else {
                                                                                        t!("rollout-detail-pushed", dispatched: r.dispatched, cohort: r.cohort_size)
                                                                                    };
                                                                                    let is_err = r.dispatched == 0;
                                                                                    request_status.set(Some((sid, msg, is_err)));
                                                                                }
                                                                                Err(e) => {
                                                                                    request_status.set(Some((sid, e.to_string(), true)));
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                },
                                                                {t!("rollout-detail-request-assessment")}
                                                            }
                                                            if has_version_update {
                                                                button { class: "btn btn-xs btn-warn-soft",
                                                                    onclick: {
                                                                        let sid = stage_id_str.clone();
                                                                        move |_| {
                                                                            let sid = sid.clone();
                                                                            request_status.set(Some((sid.clone(), t!("rollout-detail-triggering"), false)));
                                                                            async move {
                                                                                match trigger_stage_self_update(StagePushInput { stage_id: stage_id_uuid }).await {
                                                                                    Ok(r) => {
                                                                                        let msg = if r.dispatched == 0 {
                                                                                            t!("rollout-detail-push-none", cohort: r.cohort_size)
                                                                                        } else {
                                                                                            t!("rollout-detail-push-result", dispatched: r.dispatched, cohort: r.cohort_size)
                                                                                        };
                                                                                        request_status.set(Some((sid, msg, r.dispatched == 0)));
                                                                                    }
                                                                                    Err(e) => {
                                                                                        request_status.set(Some((sid, e.to_string(), true)));
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    },
                                                                    {t!("rollout-detail-trigger-self-update")}
                                                                }
                                                            }
                                                            if has_nixpkgs_update {
                                                                button { class: "btn btn-xs btn-accent",
                                                                    onclick: {
                                                                        let sid = stage_id_str.clone();
                                                                        move |_| {
                                                                            let sid = sid.clone();
                                                                            request_status.set(Some((sid.clone(), t!("rollout-detail-triggering"), false)));
                                                                            async move {
                                                                                match trigger_stage_sync_nixpkgs(StagePushInput { stage_id: stage_id_uuid }).await {
                                                                                    Ok(r) => {
                                                                                        let msg = if r.dispatched == 0 {
                                                                                            t!("rollout-detail-push-none", cohort: r.cohort_size)
                                                                                        } else {
                                                                                            t!("rollout-detail-push-result", dispatched: r.dispatched, cohort: r.cohort_size)
                                                                                        };
                                                                                        request_status.set(Some((sid, msg, r.dispatched == 0)));
                                                                                    }
                                                                                    Err(e) => {
                                                                                        request_status.set(Some((sid, e.to_string(), true)));
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    },
                                                                    {t!("rollout-detail-trigger-sync-nixpkgs")}
                                                                }
                                                            }
                                                            Link {
                                                                to: Route::FleetDashboard { stage_id: Some(stage_id_str.clone()) },
                                                                class: "btn btn-xs btn-secondary",
                                                                title: "Open the fleet dashboard filtered to this stage's cohort",
                                                                {t!("rollout-detail-view-fleet")}
                                                            }
                                                        }
                                                    }
                                                    // Inline feedback for the just-clicked button. Render
                                                    // only when the latest request targeted *this* stage so
                                                    // each card carries its own status.
                                                    {
                                                        let status_for_this_stage = request_status
                                                            .read()
                                                            .as_ref()
                                                            .filter(|(sid, _, _)| sid == &stage_id_str)
                                                            .map(|(_, msg, is_err)| (msg.clone(), *is_err));
                                                        rsx! {
                                                            if let Some((msg, is_err)) = status_for_this_stage {
                                                                {
                                                                    let cls = if is_err {
                                                                        "mt-2 text-xs text-danger-strong"
                                                                    } else {
                                                                        "mt-2 text-xs text-fg"
                                                                    };
                                                                    rsx! {
                                                                        p { class: "{cls}", "{msg}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    if let Some(h) = stage.health.as_ref() {
                                                        div { class: "flex flex-wrap gap-4 text-xs text-fg",
                                                            span {
                                                                span { class: "font-medium", {t!("rollout-detail-cohort")} }
                                                                "{h.cohort_size}"
                                                            }
                                                            span {
                                                                span { class: "font-medium", {t!("rollout-detail-heartbeats")} }
                                                                "{h.heartbeat_fresh_pct}%"
                                                            }
                                                            {
                                                                let mut pairs: Vec<(&String, &u8)> = h.probe_ok_pct.iter().collect();
                                                                pairs.sort_by(|a, b| a.0.cmp(b.0));
                                                                rsx! {
                                                                    for (svc, pct) in pairs {
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-service-probe", service: svc.clone())} }
                                                                            "{pct}%"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if !h.reasons.is_empty() {
                                                            ul { class: "mt-2 text-xs text-danger-strong list-disc list-inside",
                                                                for reason in h.reasons.iter() {
                                                                    li { "{reason}" }
                                                                }
                                                            }
                                                        }
                                                        // Per-service probe aggregates over the gate window.
                                                        if !h.probe_stats.is_empty() {
                                                            div { class: "mt-3 overflow-x-auto",
                                                                table { class: "min-w-full text-xs",
                                                                    thead {
                                                                        tr { class: "text-left text-fg-muted",
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-service")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-ok-total")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-avg-ms")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-ttft")} }
                                                                            th { class: "py-1 pr-3 font-medium", {t!("rollout-detail-col-tokens-out")} }
                                                                            th { class: "py-1 font-medium", {t!("rollout-detail-col-last-failure")} }
                                                                        }
                                                                    }
                                                                    tbody { class: "text-fg font-mono",
                                                                        {
                                                                            let mut stat_pairs: Vec<(&String, &ProbeStatsView)> = h.probe_stats.iter().collect();
                                                                            stat_pairs.sort_by(|a, b| a.0.cmp(b.0));
                                                                            rsx! {
                                                                                for (svc, ps) in stat_pairs {
                                                                                    {
                                                                                        let dur = ps.avg_duration_ms.map(|v| format!("{v}")).unwrap_or_else(|| t!("em-dash"));
                                                                                        let ttft = ps.avg_first_token_ms.map(|v| format!("{v}ms")).unwrap_or_else(|| t!("em-dash"));
                                                                                        let tokens = ps.avg_tokens_out.map(|v| format!("{v}")).unwrap_or_else(|| t!("em-dash"));
                                                                                        let failure = match (ps.last_failure_at, &ps.last_error_class) {
                                                                                            (Some(at), Some(cls)) => format!("{} ({cls})", at.format("%H:%M:%S")),
                                                                                            (Some(at), None) => at.format("%H:%M:%S").to_string(),
                                                                                            _ => t!("em-dash"),
                                                                                        };
                                                                                        rsx! {
                                                                                            tr {
                                                                                                td { class: "py-1 pr-3 font-sans font-medium", "{svc}" }
                                                                                                td { class: "py-1 pr-3", "{ps.ok_count}/{ps.total_runs}" }
                                                                                                td { class: "py-1 pr-3", "{dur}" }
                                                                                                td { class: "py-1 pr-3", "{ttft}" }
                                                                                                td { class: "py-1 pr-3", "{tokens}" }
                                                                                                td { class: "py-1 text-fg-muted font-sans", "{failure}" }
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
                                                        // Cohort sample summary — derived from the most recent
                                                        // heartbeat sample of every reporting instance.
                                                        if let Some(s) = h.sample_summary.as_ref() {
                                                            {
                                                                let disk = s.max_disk_used_pct.map(|v| format!("{v}%")).unwrap_or_else(|| t!("em-dash"));
                                                                let gpu = s.gpu_avg_util_pct.map(|v| format!("{v}%")).unwrap_or_else(|| t!("em-dash"));
                                                                let cpu = format!("{:.2}", s.avg_cpu_load_1m);
                                                                rsx! {
                                                                    div { class: "mt-3 flex flex-wrap gap-4 text-xs text-fg",
                                                                        span {
                                                                            span { class: "font-medium", {t!("rollout-detail-samples")} }
                                                                            "{s.reporting_instances}"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-cpu-load")} }
                                                                            "{cpu}"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-mem")} }
                                                                            "{s.avg_mem_used_pct}%"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-max-disk")} }
                                                                            "{disk}"
                                                                        }
                                                                        span { class: "font-mono",
                                                                            span { class: "font-medium font-sans", {t!("rollout-detail-gpu-util")} }
                                                                            "{gpu}"
                                                                        }
                                                                        if s.thermal_alerts > 0 {
                                                                            span { class: "text-warn-strong font-medium",
                                                                                {t!("rollout-detail-thermal-alerts", count: s.thermal_alerts)}
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

                                    // ── Inline gate editor ──
                                    // Renders when edit_gate's target matches
                                    // this stage_id. Kept under the gate panel
                                    // so save/cancel lands in the operator's
                                    // natural reading flow.
                                    {
                                        let editing_here = edit_gate
                                            .read()
                                            .as_ref()
                                            .map(|(sid, _, _)| sid == &stage_id_str)
                                            .unwrap_or(false);
                                        let err = if editing_here {
                                            edit_gate_error.read().clone()
                                        } else {
                                            None
                                        };
                                        rsx! {
                                            if editing_here {
                                                div { class: "mt-3 pt-3 border-t border-line-soft",
                                                    div { class: "flex items-center justify-between mb-2",
                                                        span { class: "text-sm font-semibold text-fg-strong",
                                                            {t!("rollout-detail-gate-config")}
                                                        }
                                                        button { class: "text-xs text-fg-muted hover:underline",
                                                            onclick: move |_| {
                                                                edit_gate.set(None);
                                                                edit_gate_error.set(None);
                                                            },
                                                            {t!("cancel")}
                                                        }
                                                    }

                                                    // Read snapshot for display; writes flow through
                                                    // edit_gate.write() on each input change.
                                                    {
                                                        let snap = edit_gate
                                                            .read()
                                                            .as_ref()
                                                            .map(|(_, g, a)| (g.clone(), *a))
                                                            .unwrap_or_else(|| (HealthGateInput::default(), false));
                                                        let (g, apply_all) = snap;
                                                        rsx! {
                                                            div { class: "flex items-center gap-2 mb-3",
                                                                input {
                                                                    r#type: "checkbox",
                                                                    checked: g.enabled,
                                                                    class: "rounded border-line text-brand focus:ring-brand",
                                                                    onchange: move |e| {
                                                                        let mut w = edit_gate.write();
                                                                        if let Some(t) = w.as_mut() {
                                                                            t.1.enabled = e.value() == "true";
                                                                        }
                                                                    },
                                                                }
                                                                label { class: "text-xs text-fg-strong",
                                                                    {t!("rollout-detail-gate-enabled")}
                                                                }
                                                            }

                                                            if g.enabled {
                                                                div { class: "grid grid-cols-1 sm:grid-cols-3 gap-3 mb-3",
                                                                    div {
                                                                        label { class: "block text-xs font-medium text-fg mb-1",
                                                                            {t!("rollout-form-heartbeat-pct")}
                                                                        }
                                                                        input {
                                                                            r#type: "number", min: "0", max: "100",
                                                                            class: "input input-sm",
                                                                            value: "{g.min_heartbeat_fresh_pct}",
                                                                            oninput: move |e| {
                                                                                if let Ok(v) = e.value().parse::<u8>() {
                                                                                    let mut w = edit_gate.write();
                                                                                    if let Some(t) = w.as_mut() {
                                                                                        t.1.min_heartbeat_fresh_pct = v.min(100);
                                                                                    }
                                                                                }
                                                                            },
                                                                        }
                                                                    }
                                                                    div {
                                                                        label { class: "block text-xs font-medium text-fg mb-1",
                                                                            {t!("rollout-detail-freshness-window")}
                                                                        }
                                                                        input {
                                                                            r#type: "number", min: "10",
                                                                            class: "input input-sm",
                                                                            value: "{g.heartbeat_freshness_secs}",
                                                                            oninput: move |e| {
                                                                                if let Ok(v) = e.value().parse::<u32>() {
                                                                                    let mut w = edit_gate.write();
                                                                                    if let Some(t) = w.as_mut() {
                                                                                        t.1.heartbeat_freshness_secs = v.max(10);
                                                                                    }
                                                                                }
                                                                            },
                                                                        }
                                                                    }
                                                                    div {
                                                                        label { class: "block text-xs font-medium text-fg mb-1",
                                                                            {t!("rollout-detail-grace-period")}
                                                                        }
                                                                        input {
                                                                            r#type: "number", min: "0",
                                                                            class: "input input-sm",
                                                                            value: "{g.grace_period_secs}",
                                                                            oninput: move |e| {
                                                                                if let Ok(v) = e.value().parse::<u32>() {
                                                                                    let mut w = edit_gate.write();
                                                                                    if let Some(t) = w.as_mut() {
                                                                                        t.1.grace_period_secs = v;
                                                                                    }
                                                                                }
                                                                            },
                                                                        }
                                                                    }
                                                                }

                                                                div { class: "mb-3",
                                                                    label { class: "block text-xs font-medium text-fg mb-1",
                                                                        {t!("rollout-form-probe-thresholds")}
                                                                    }
                                                                    {
                                                                        let rows: Vec<(usize, String, u8)> = g
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
                                                                                            let mut w = edit_gate.write();
                                                                                            if let Some(t) = w.as_mut() {
                                                                                                if let Some(row) = t.1.probe_thresholds.get_mut(idx) {
                                                                                                    row.0 = e.value();
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                    }
                                                                                    input {
                                                                                        r#type: "number", min: "0", max: "100",
                                                                                        class: "input input-sm w-20",
                                                                                        value: "{pct}",
                                                                                        oninput: move |e| {
                                                                                            if let Ok(v) = e.value().parse::<u8>() {
                                                                                                let mut w = edit_gate.write();
                                                                                                if let Some(t) = w.as_mut() {
                                                                                                    if let Some(row) = t.1.probe_thresholds.get_mut(idx) {
                                                                                                        row.1 = v.min(100);
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                    }
                                                                                    span { class: "text-xs text-fg-muted", {t!("rollout-form-pct-symbol")} }
                                                                                    button { class: "link-danger text-xs",
                                                                                        onclick: move |_| {
                                                                                            let mut w = edit_gate.write();
                                                                                            if let Some(t) = w.as_mut() {
                                                                                                if idx < t.1.probe_thresholds.len() {
                                                                                                    t.1.probe_thresholds.remove(idx);
                                                                                                }
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
                                                                            let mut w = edit_gate.write();
                                                                            if let Some(t) = w.as_mut() {
                                                                                t.1.probe_thresholds.push((String::new(), 90));
                                                                            }
                                                                        },
                                                                        {t!("rollout-form-add-service")}
                                                                    }
                                                                }
                                                            }

                                                            // Apply-to-all + Save row.
                                                            div { class: "flex items-center gap-3 mt-3",
                                                                div { class: "flex items-center gap-2",
                                                                    input {
                                                                        r#type: "checkbox",
                                                                        checked: apply_all,
                                                                        class: "rounded border-line text-brand focus:ring-brand",
                                                                        onchange: move |e| {
                                                                            let mut w = edit_gate.write();
                                                                            if let Some(t) = w.as_mut() {
                                                                                t.2 = e.value() == "true";
                                                                            }
                                                                        },
                                                                    }
                                                                    label { class: "text-xs text-fg",
                                                                        {t!("rollout-detail-apply-all")}
                                                                    }
                                                                }
                                                                button { class: "ml-auto btn btn-xs btn-primary",
                                                                    onclick: move |_| {
                                                                        let snap = edit_gate
                                                                            .read()
                                                                            .as_ref()
                                                                            .map(|(sid, g, a)| (sid.clone(), g.clone(), *a));
                                                                        async move {
                                                                            let Some((sid, g, apply_all)) = snap else { return; };
                                                                            let stage_id: Uuid = match sid.parse() {
                                                                                Ok(v) => v,
                                                                                Err(e) => {
                                                                                    edit_gate_error.set(Some(e.to_string()));
                                                                                    return;
                                                                                }
                                                                            };
                                                                            let input = StageGateUpdateInput {
                                                                                stage_id,
                                                                                gate: Some(g.into()),
                                                                                apply_to_all: apply_all,
                                                                            };
                                                                            match update_stage_gate(input).await {
                                                                                Ok(()) => {
                                                                                    edit_gate.set(None);
                                                                                    edit_gate_error.set(None);
                                                                                    detail.restart();
                                                                                }
                                                                                Err(e) => {
                                                                                    edit_gate_error.set(Some(e.to_string()));
                                                                                }
                                                                            }
                                                                        }
                                                                    },
                                                                    {t!("save")}
                                                                }
                                                            }
                                                            if let Some(msg) = err.as_ref() {
                                                                p { class: "mt-2 text-xs text-danger-strong",
                                                                    "{msg}"
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
                    }
                }

                // Target
                SectionHeading { class: "mb-2", {t!("rollout-detail-target")} }
                div { class: "bg-surface-2 p-4 rounded text-sm space-y-1",
                    if let Some(ver) = &info.target_version {
                        p { span { class: "font-medium", {t!("rollout-detail-version-label")} } "{ver}" }
                    }
                    if let Some(commit) = &info.nixpkgs_commit {
                        {
                            let short: String = commit.chars().take(12).collect();
                            let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{commit}");
                            let sha_for_count = commit.clone();
                            let count_res = use_resource(move || {
                                let s = sha_for_count.clone();
                                async move { get_nixpkgs_commit_count_rollout(s).await.ok().flatten() }
                            });
                            let count_label = count_res.read().as_ref()
                                .and_then(|n| n.as_ref())
                                .map(|n| format!(" #{n}"))
                                .unwrap_or_default();
                            rsx! {
                                p {
                                    span { class: "font-medium", {t!("rollout-detail-nixpkgs-label")} }
                                    a { class: "font-mono text-sm hover:text-brand",
                                        href: "{url}",
                                        target: "_blank",
                                        title: "{commit}",
                                        "{short}{count_label}"
                                    }
                                }
                            }
                        }
                    }
                }

                // Delivered clusters — fetched this rollout's target and
                // stay pinned to it for the rollout's lifetime, even while
                // paused/gated. Hidden until the first delivery.
                if !info.deliveries.is_empty() {
                    SectionHeading { class: "mb-2 mt-4",
                        {t!("rollout-detail-delivered", count: info.deliveries.len())}
                    }
                    div { class: "bg-surface-2 p-4 rounded text-sm",
                        p { class: "text-xs text-fg-muted mb-2",
                            {t!("rollout-detail-delivered-help")}
                        }
                        div { class: "flex flex-wrap gap-x-4 gap-y-1",
                            for d in &info.deliveries {
                                {
                                    let ts = d.delivered_at.format("%Y-%m-%d %H:%M").to_string();
                                    rsx! {
                                        span {
                                            Link {
                                                class: "hover:text-brand font-medium",
                                                to: Route::ClusterDetail { id: d.cluster_id.to_string() },
                                                "{d.cluster_name}"
                                            }
                                            span { class: "text-xs text-fg-muted font-mono ml-1", "{ts}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Baseline — shown once the rollout has started so operators
                // can see exactly what a rollback would restore. Hidden for
                // pending rollouts where nothing's been captured yet.
                if info.baseline_version.is_some() || info.baseline_nixpkgs_commit.is_some() {
                    SectionHeading { class: "mb-2 mt-4", {t!("rollout-detail-rollback-baseline")} }
                    div { class: "bg-surface-2 p-4 rounded text-sm space-y-1",
                        if let Some(ver) = &info.baseline_version {
                            p { span { class: "font-medium", {t!("rollout-detail-version-label")} } "{ver}" }
                        }
                        if let Some(commit) = &info.baseline_nixpkgs_commit {
                            {
                                let short: String = commit.chars().take(12).collect();
                                let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{commit}");
                                let sha_for_count = commit.clone();
                                let count_res = use_resource(move || {
                                    let s = sha_for_count.clone();
                                    async move { get_nixpkgs_commit_count_rollout(s).await.ok().flatten() }
                                });
                                let count_label = count_res.read().as_ref()
                                    .and_then(|n| n.as_ref())
                                    .map(|n| format!(" #{n}"))
                                    .unwrap_or_default();
                                rsx! {
                                    p {
                                        span { class: "font-medium", {t!("rollout-detail-nixpkgs-label")} }
                                        a { class: "font-mono text-sm hover:text-brand",
                                            href: "{url}",
                                            target: "_blank",
                                            title: "{commit}",
                                            "{short}{count_label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
