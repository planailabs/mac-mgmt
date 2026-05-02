use dioxus::prelude::*;

use super::pill::{Pill, PillVariant};

#[derive(Clone, Copy, PartialEq)]
pub enum StageStatus {
    Completed,
    InProgress,
    Pending,
}

/// One stage in a rollout. `fleet_total` / `fleet_filled` drive the
/// mini "fleet of devices" indicator under each circle (small bars,
/// filled = at-target version).
#[derive(Clone, PartialEq)]
pub struct StageItem {
    pub name: String,
    pub status: StageStatus,
    pub fleet_total: usize,
    pub fleet_filled: usize,
    /// Free-form summary line under the fleet — e.g. "8/12 online · nixpkgs 8/12".
    pub summary: String,
}

/// Horizontal stage timeline used by Rollouts. Renders an N-column
/// row of circles connected by a track that fills with brand color
/// up to the current in-progress stage.
#[component]
pub fn StageTimeline(stages: Vec<StageItem>) -> Element {
    let n = stages.len().max(1);
    // The track sits 22px from the top (centered through the 44px circles)
    // and spans from the first to the last circle's centerline. We
    // approximate that with 16% inset on each side so it looks centered
    // regardless of column count.
    let total_completed = stages
        .iter()
        .filter(|s| s.status == StageStatus::Completed)
        .count();
    let in_progress_idx = stages
        .iter()
        .position(|s| s.status == StageStatus::InProgress);
    let progress_pct = match in_progress_idx {
        Some(_) => {
            // Half a step into the in-progress one
            let cells = (n - 1).max(1) as f64;
            ((total_completed as f64) + 0.5) / cells * 100.0
        }
        None => (total_completed as f64) / ((n - 1).max(1) as f64) * 100.0,
    };
    let fill_width = format!("{:.1}%", progress_pct.clamp(0.0, 100.0));
    let grid = format!("grid-template-columns: repeat({n}, minmax(0, 1fr));");

    rsx! {
        div { class: "card card-pad",
            div { class: "relative grid", style: "{grid}",
                // base track
                div {
                    class: "absolute top-[22px] left-[8%] right-[8%] h-[2px] bg-line-soft pointer-events-none",
                }
                // filled track
                div {
                    class: "absolute top-[22px] left-[8%] h-[2px] bg-brand pointer-events-none",
                    style: "width: calc((100% - 16%) * {progress_pct:.2} / 100);",
                }
                for (i, s) in stages.iter().cloned().enumerate() {
                    StageCell { key: "{i}", index: i, stage: s }
                }
            }
            // Suppress unused binding warning on fill_width if any
            div { class: "hidden", "{fill_width}" }
        }
    }
}

#[component]
fn StageCell(index: usize, stage: StageItem) -> Element {
    let done = stage.status == StageStatus::Completed;
    let in_progress = stage.status == StageStatus::InProgress;

    let circle_cls = if done {
        "bg-brand text-fg-invert border-0"
    } else if in_progress {
        "bg-brand-soft text-brand border-2 border-brand"
    } else {
        "bg-surface-2 text-fg-faint border-2 border-line-soft"
    };

    let pill_variant = if done {
        PillVariant::Ok
    } else if in_progress {
        PillVariant::Accent
    } else {
        PillVariant::Muted
    };
    let pill_label = match stage.status {
        StageStatus::Completed => "completed",
        StageStatus::InProgress => "in progress",
        StageStatus::Pending => "pending",
    };

    rsx! {
        div { class: "relative text-center px-3",
            // Circle
            div {
                class: "w-11 h-11 rounded-full mx-auto mb-3.5 flex items-center justify-center relative z-10 {circle_cls}",
                style: if in_progress { "box-shadow: 0 0 0 4px rgb(var(--c-brand-soft));" } else { "" },
                if done {
                    svg {
                        width: "18", height: "18", "viewBox": "0 0 18 18",
                        path {
                            d: "M3 9 L7 13 L15 5",
                            stroke: "currentColor",
                            "stroke-width": "2.4",
                            fill: "none",
                            "stroke-linecap": "round",
                            "stroke-linejoin": "round",
                        }
                    }
                } else {
                    span { class: "text-sm font-semibold font-mono", "{index}" }
                }
            }
            // Stage label kicker
            div { class: "kicker mb-1", "Stage {index}" }
            // Stage name
            div { class: "text-base font-semibold text-fg-strong mb-1.5 font-mono", "{stage.name}" }
            // Status pill
            Pill { variant: pill_variant, "{pill_label}" }
            // Mini fleet of devices
            if stage.fleet_total > 0 {
                FleetMini {
                    total: stage.fleet_total,
                    filled: stage.fleet_filled,
                }
            }
            // Summary
            div { class: "mt-3 font-mono text-[11.5px] text-fg-muted", "{stage.summary}" }
        }
    }
}

#[component]
fn FleetMini(total: usize, filled: usize) -> Element {
    rsx! {
        div { class: "mt-4 flex justify-center gap-1 flex-wrap",
            for j in 0..total {
                {
                    let is_filled = j < filled;
                    let cls = if is_filled {
                        "w-3.5 h-[18px] rounded-sm bg-brand"
                    } else {
                        "w-3.5 h-[18px] rounded-sm bg-line-soft opacity-60"
                    };
                    rsx! { div { key: "{j}", class: "{cls}" } }
                }
            }
        }
    }
}
