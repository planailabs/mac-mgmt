//! Dioxus components shared by consumers of action templates: a parameter
//! form generated from the template's `inputs`, a live run progress view
//! (current step centered, progress bar, toggleable debug log), and a run
//! report viewer. Styling follows plan-ai-design semantic classes plus the
//! consumer's Tailwind tokens (add this crate to the Tailwind `content` glob).

use std::collections::HashMap;

use dioxus::prelude::*;
use plan_ai_design::{Badge, BadgeVariant, Button, ButtonKind, ButtonVariant, FormField};
use serde_json::{Map, Value};

use crate::report::{RunReport, RunStatus, StepStatus};
use crate::spec::{IdOption, InputSpec, InputType};

/// Parameter form generated from a template's declared inputs (in order).
/// `id_options` holds the picker choices per `id`-typed input name.
/// Emits the collected params map on submit; defaults and coercion are the
/// engine's job, so unset optional fields are simply omitted.
#[component]
pub fn ActionParamsForm(
    inputs: Vec<(String, InputSpec)>,
    id_options: HashMap<String, Vec<IdOption>>,
    #[props(default)] busy: bool,
    on_submit: EventHandler<Map<String, Value>>,
) -> Element {
    let seeded = {
        let inputs = inputs.clone();
        move || {
            let mut m = Map::new();
            for (name, spec) in &inputs {
                if let Some(default) = &spec.default {
                    m.insert(name.clone(), default.clone());
                }
            }
            m
        }
    };
    let mut values = use_signal(seeded);

    rsx! {
        form {
            onsubmit: move |evt| {
                evt.prevent_default();
                on_submit.call(values.read().clone());
            },
            for (name, spec) in inputs.iter() {
                {
                    let label = spec.label.clone().unwrap_or_else(|| name.clone());
                    let label = if spec.required { format!("{label} *") } else { label };
                    let key = name.clone();
                    let current = values.read().get(name).cloned();
                    let field = param_field(spec, &key, current, &id_options, &mut values);
                    rsx! {
                        FormField {
                            label,
                            help: spec.description.clone(),
                            {field}
                        }
                    }
                }
            }
            Button {
                kind: ButtonKind::Submit,
                variant: ButtonVariant::Primary,
                disabled: busy,
                if busy { "Running…" } else { "Execute" }
            }
        }
    }
}

fn param_field(
    spec: &InputSpec,
    key: &str,
    current: Option<Value>,
    id_options: &HashMap<String, Vec<IdOption>>,
    values: &mut Signal<Map<String, Value>>,
) -> Element {
    let mut values = *values;
    let key = key.to_string();
    match spec.ty {
        InputType::String => {
            let text = current.as_ref().and_then(Value::as_str).unwrap_or("").to_string();
            rsx! {
                input {
                    class: "input",
                    r#type: "text",
                    value: "{text}",
                    oninput: move |e| {
                        values.write().insert(key.clone(), Value::String(e.value()));
                    },
                }
            }
        }
        InputType::Int => {
            let text = current
                .as_ref()
                .map(|v| match v {
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            rsx! {
                input {
                    class: "input",
                    r#type: "number",
                    value: "{text}",
                    oninput: move |e| {
                        // Keep raw text; the engine coerces "42" -> 42 and
                        // rejects non-integers with a proper message.
                        values.write().insert(key.clone(), Value::String(e.value()));
                    },
                }
            }
        }
        InputType::Bool => {
            let checked = current.as_ref().and_then(Value::as_bool).unwrap_or(false);
            rsx! {
                input {
                    class: "h-4 w-4",
                    r#type: "checkbox",
                    checked,
                    onchange: move |e| {
                        values.write().insert(key.clone(), Value::Bool(e.checked()));
                    },
                }
            }
        }
        InputType::List => {
            let selected = current.as_ref().and_then(Value::as_str).unwrap_or("").to_string();
            let options = spec.options.clone().unwrap_or_default();
            rsx! {
                select {
                    class: "input",
                    value: "{selected}",
                    onchange: move |e| {
                        values.write().insert(key.clone(), Value::String(e.value()));
                    },
                    option { value: "", disabled: true, selected: selected.is_empty(), "— select —" }
                    for opt in options {
                        option { value: "{opt}", selected: selected == opt, "{opt}" }
                    }
                }
            }
        }
        InputType::Id => {
            let selected = current.as_ref().and_then(Value::as_str).unwrap_or("").to_string();
            let options = id_options.get(&key).cloned().unwrap_or_default();
            rsx! {
                select {
                    class: "input",
                    value: "{selected}",
                    onchange: move |e| {
                        values.write().insert(key.clone(), Value::String(e.value()));
                    },
                    option { value: "", disabled: true, selected: selected.is_empty(), "— select —" }
                    for opt in options {
                        option { value: "{opt.value}", selected: selected == opt.value, "{opt.label}" }
                    }
                }
            }
        }
    }
}

/// Live run view: while running, the current step name centered over a
/// progress bar; a toggleable per-step debug log; the full [`RunReportView`]
/// once the run finishes.
#[component]
pub fn RunProgressView(status: RunStatus) -> Element {
    let mut show_log = use_signal(|| false);

    let total = status.total_steps.max(1);
    let pct = if status.done {
        100.0
    } else {
        (status.current_step as f32 / total as f32) * 100.0
    };

    rsx! {
        div { class: "space-y-4",
            if status.done {
                if let Some(report) = &status.report {
                    RunReportView { report: report.clone() }
                }
            } else {
                div { class: "flex flex-col items-center gap-3 py-8",
                    div { class: "text-lg font-medium text-fg-strong text-center animate-pulse",
                        if status.current_step_name.is_empty() {
                            "Queued…"
                        } else {
                            "{status.current_step_name}"
                        }
                    }
                    div { class: "text-xs text-fg-muted",
                        "step {status.current_step + 1} / {status.total_steps}"
                    }
                    div { class: "w-full max-w-md h-2 rounded-full bg-surface-3 overflow-hidden",
                        div {
                            class: "h-full rounded-full bg-brand transition-all duration-300",
                            style: "width: {pct}%",
                        }
                    }
                }
            }
            div {
                Button {
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| {
                        let showing = *show_log.read();
                        show_log.set(!showing);
                    },
                    if *show_log.read() { "Hide debug log" } else { "Show debug log" }
                }
                if *show_log.read() {
                    div { class: "mt-2 max-h-80 overflow-y-auto rounded border border-line bg-surface-2 p-3 font-mono text-xs space-y-0.5",
                        if status.events.is_empty() {
                            div { class: "text-fg-muted", "no events yet" }
                        }
                        for event in status.events.iter() {
                            div { key: "{event.seq}",
                                span { class: "text-fg-faint mr-2", "[{event.step_index + 1}]" }
                                span { class: "text-fg-muted", "{event.message}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Per-step outcome viewer: status badge, collapsible rendered inputs/output,
/// error text, and the final variables.
#[component]
pub fn RunReportView(report: RunReport) -> Element {
    rsx! {
        div { class: "space-y-2",
            div { class: "flex items-center gap-2",
                if report.ok {
                    Badge { variant: BadgeVariant::Success, "run ok" }
                } else {
                    Badge { variant: BadgeVariant::Danger, "run failed" }
                }
            }
            for (i, step) in report.steps.iter().enumerate() {
                details { class: "rounded border border-line bg-surface-2",
                    summary { class: "flex items-center gap-2 cursor-pointer select-none px-3 py-2",
                        match step.status {
                            StepStatus::Ok => rsx! { Badge { variant: BadgeVariant::Success, "ok" } },
                            StepStatus::Skipped => rsx! { Badge { variant: BadgeVariant::Neutral, "skipped" } },
                            StepStatus::Failed => rsx! { Badge { variant: BadgeVariant::Danger, "failed" } },
                        }
                        span { class: "text-sm text-fg-strong", "{i + 1}. {step.name}" }
                        span { class: "text-xs text-fg-muted font-mono", "{step.action}" }
                    }
                    div { class: "px-3 pb-3 space-y-2",
                        if let Some(inputs) = &step.inputs {
                            JsonBlock { label: "inputs", value: inputs.clone() }
                        }
                        if let Some(output) = &step.output {
                            JsonBlock { label: "output", value: output.clone() }
                        }
                        if let Some(error) = &step.error {
                            div { class: "text-sm text-danger", "{error}" }
                        }
                    }
                }
            }
            if !report.variables.is_empty() {
                details { class: "rounded border border-line bg-surface-2",
                    summary { class: "cursor-pointer select-none px-3 py-2 text-sm text-fg-muted",
                        "final variables"
                    }
                    div { class: "px-3 pb-3",
                        JsonBlock { label: "variables", value: Value::Object(report.variables.clone()) }
                    }
                }
            }
        }
    }
}

#[component]
fn JsonBlock(#[props(into)] label: String, value: Value) -> Element {
    let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    rsx! {
        div {
            div { class: "text-xs text-fg-faint mb-1", "{label}" }
            pre { class: "font-mono text-xs bg-surface-3 rounded p-2 overflow-x-auto whitespace-pre-wrap",
                "{pretty}"
            }
        }
    }
}
