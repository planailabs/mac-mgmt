use dioxus::prelude::*;

use super::pill::{Dot, Pill, PillVariant};

#[derive(Clone, Copy, PartialEq)]
pub enum TraceStatus {
    Done,
    InProgress,
    Pending,
}

#[derive(Clone, PartialEq)]
pub struct TraceStep {
    pub status: TraceStatus,
    pub text: String,
}

/// Hero-style "session in progress" card used on the Healer page.
/// Shows: status pills row, large title, mono trace with ✓/▸/○
/// glyphs, progress bar with step counter.
///
/// The accent gradient + outer halo is the only place in the design
/// language that uses a strong shadow — calling out the live action.
#[component]
pub fn ActiveSessionCard(
    title: String,
    #[props(default, into)] subtitle: Option<String>,
    trace: Vec<TraceStep>,
    /// 0..=trace.len() — index of the in-progress step. Drives the
    /// progress bar fill percentage and the "step N of M" line.
    current_step: usize,
    /// Optional duration label — e.g. "00:00:32".
    #[props(default, into)] duration: Option<String>,
    /// Right-side action (typically an "Open session" button). Pass any
    /// element; rendered as-is.
    #[props(default)] action: Option<Element>,
) -> Element {
    let total = trace.len().max(1);
    let pct = (current_step as f64 / total as f64 * 100.0).clamp(0.0, 100.0);
    let pct_str = format!("{:.0}%", pct);

    rsx! {
        div {
            class: "rounded-2xl p-[22px] relative overflow-hidden border border-brand bg-surface",
            style: "background: linear-gradient(135deg, rgb(var(--c-brand-soft)) 0%, rgb(var(--c-surface)) 70%); box-shadow: 0 0 0 3px rgb(var(--c-brand-soft));",

            // Header row
            div { class: "flex justify-between items-start mb-3.5 gap-3",
                div { class: "flex items-center gap-2.5 min-w-0",
                    div {
                        class: "w-9 h-9 rounded-[10px] bg-brand text-fg-invert flex items-center justify-center shrink-0",
                        svg {
                            width: "18", height: "18", "viewBox": "0 0 18 18",
                            path {
                                d: "M9 2 L11 7 L16 7.5 L12 11 L13 16 L9 13.5 L5 16 L6 11 L2 7.5 L7 7 Z",
                                fill: "currentColor",
                            }
                        }
                    }
                    div { class: "min-w-0",
                        div { class: "flex items-center gap-2 mb-1 flex-wrap",
                            Pill { variant: PillVariant::Accent, "active" }
                            Pill { variant: PillVariant::Muted, mono: true, "auto" }
                            if let Some(s) = subtitle {
                                span { class: "text-fg-muted text-xs", "{s}" }
                            }
                        }
                        div { class: "text-fg-strong text-base font-semibold tracking-tight truncate",
                            "{title}"
                        }
                    }
                }
                if let Some(a) = action { {a} }
            }

            // Trace block
            div {
                class: "rounded-lg p-4 font-mono text-[11.5px] text-fg-muted leading-[1.7] border border-line bg-surface-2",
                for (i, step) in trace.iter().cloned().enumerate() {
                    {
                        let (glyph, glyph_cls, line_cls) = match step.status {
                            TraceStatus::Done       => ("✓", "text-success", "text-fg-muted"),
                            TraceStatus::InProgress => ("▸", "text-brand",   "text-fg-strong"),
                            TraceStatus::Pending    => ("○", "text-fg-faint","text-fg-faint"),
                        };
                        rsx! {
                            div { key: "{i}",
                                span { class: "{glyph_cls}", "{glyph}" }
                                " "
                                span { class: "{line_cls}", "{step.text}" }
                            }
                        }
                    }
                }
            }

            // Progress row
            div { class: "mt-3 flex items-center gap-3",
                div { class: "flex items-center gap-2 text-fg-muted text-[11.5px] font-mono shrink-0",
                    Dot { variant: PillVariant::Accent }
                    "step {current_step} of {total}"
                    if let Some(d) = duration {
                        " · {d}"
                    }
                }
                div { class: "flex-1 h-1 bg-line-soft rounded-sm overflow-hidden",
                    div { class: "h-full bg-brand rounded-sm", style: "width: {pct_str};" }
                }
                span { class: "text-[11.5px] font-mono text-fg-muted shrink-0", "{pct_str}" }
            }
        }
    }
}
