//! Run page for one action template: parameter form (generated from the
//! template's inputs), live progress via long-polling, and run history.

use std::collections::HashMap;

use dioxus::prelude::*;
use plan_ai_actions::report::RunStatus;
use plan_ai_actions::ui::{ActionParamsForm, RunProgressView, RunReportView};

use super::ui::{Badge, BadgeVariant, Card, PageHeader, SectionHeading};
use crate::api_mcp::endpoints::action_templates::{
    ActionTemplateGetInput, ActionTemplateInputOptionsInput, ActionTemplateRunStatusInput,
    ActionTemplateRunsInput, ActionTemplateStartInput, action_template_id_options,
    get_action_template, list_action_template_runs, poll_action_template_run,
    start_action_template,
};

#[component]
pub fn ActionTemplateDetail(id: String) -> Element {
    let data = use_server_future({
        let id = id.clone();
        move || {
            let id = id.clone();
            async move { get_action_template(ActionTemplateGetInput { id }).await }
        }
    })?;
    let options = use_server_future({
        let id = id.clone();
        move || {
            let id = id.clone();
            async move {
                action_template_id_options(ActionTemplateInputOptionsInput { id }).await
            }
        }
    })?;

    let mut run: Signal<Option<RunStatus>> = use_signal(|| None);
    let mut error: Signal<Option<String>> = use_signal(|| None);
    let mut history_tick: Signal<u32> = use_signal(|| 0);

    let history = use_server_future({
        let id = id.clone();
        move || {
            let id = id.clone();
            let _tick = history_tick();
            async move { list_action_template_runs(ActionTemplateRunsInput { id }).await }
        }
    })?;

    let template = match &*data.read() {
        Some(Ok(t)) => t.clone(),
        Some(Err(e)) => {
            return rsx! { div { class: "text-danger", "failed to load template: {e}" } };
        }
        None => return rsx! {},
    };
    let id_options: HashMap<_, _> = match &*options.read() {
        Some(Ok(o)) => o.options.clone().into_iter().collect(),
        _ => HashMap::new(),
    };

    let busy = run.read().as_ref().is_some_and(|s| !s.done);
    let inputs: Vec<_> = template
        .spec
        .inputs
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let on_submit = {
        let id = id.clone();
        move |params: serde_json::Map<String, serde_json::Value>| {
            let id = id.clone();
            error.set(None);
            run.set(None);
            spawn(async move {
                let started =
                    start_action_template(ActionTemplateStartInput { id: id.clone(), params })
                        .await;
                let run_id = match started {
                    Ok(res) => res.run_id,
                    Err(e) => {
                        error.set(Some(e.to_string()));
                        return;
                    }
                };
                // Long-poll loop: the server holds each request until there
                // are new events, so no client-side timer is needed. Events
                // arrive incrementally (after_seq cursor) and accumulate here.
                let mut after_seq: Option<u64> = None;
                loop {
                    match poll_action_template_run(ActionTemplateRunStatusInput {
                        id: id.clone(),
                        run_id,
                        after_seq,
                    })
                    .await
                    {
                        Ok(status) => {
                            if let Some(last) = status.events.last() {
                                after_seq = Some(last.seq);
                            }
                            let mut merged = status;
                            let prior = run
                                .read()
                                .as_ref()
                                .map(|s| s.events.clone())
                                .unwrap_or_default();
                            let mut events = prior;
                            events.extend(merged.events);
                            merged.events = events;
                            let done = merged.done;
                            run.set(Some(merged));
                            if done {
                                break;
                            }
                        }
                        Err(e) => {
                            error.set(Some(e.to_string()));
                            break;
                        }
                    }
                }
                history_tick += 1;
            });
        }
    };

    rsx! {
        PageHeader { "Action Template: {template.name}" }
        if !template.description.is_empty() {
            p { class: "mt-2 text-sm text-fg-muted", "{template.description}" }
        }

        div { class: "mt-4 grid gap-6 lg:grid-cols-2",
            Card {
                SectionHeading { "Run" }
                ActionParamsForm {
                    inputs,
                    id_options,
                    busy,
                    on_submit,
                }
                if let Some(e) = error.read().as_ref() {
                    div { class: "mt-2 text-sm text-danger", "{e}" }
                }
                if let Some(status) = run.read().clone() {
                    div { class: "mt-4",
                        RunProgressView { status }
                    }
                }
            }

            Card {
                SectionHeading { "Definition" }
                details {
                    summary { class: "cursor-pointer select-none text-sm text-fg-muted", "YAML source" }
                    pre { class: "mt-2 font-mono text-xs bg-surface-3 rounded p-3 overflow-x-auto whitespace-pre-wrap",
                        "{template.yaml}"
                    }
                }
            }
        }

        div { class: "mt-6",
            SectionHeading { "Run History" }
            Card {
                match &*history.read() {
                    Some(Ok(runs)) if runs.is_empty() => rsx! {
                        div { class: "text-sm text-fg-muted", "No runs yet" }
                    },
                    Some(Ok(runs)) => rsx! {
                        div { class: "space-y-2",
                            for row in runs.iter() {
                                details { key: "{row.run_id}", class: "rounded border border-line",
                                    summary { class: "flex items-center gap-3 cursor-pointer select-none px-3 py-2",
                                        match row.ok {
                                            Some(true) => rsx! { Badge { variant: BadgeVariant::Success, "ok" } },
                                            Some(false) => rsx! { Badge { variant: BadgeVariant::Danger, "failed" } },
                                            None => rsx! { Badge { variant: BadgeVariant::Warn, "{row.status}" } },
                                        }
                                        span { class: "text-sm text-fg-muted font-mono", "{row.created_at}" }
                                        span { class: "text-sm text-fg-muted", "{row.subject}" }
                                    }
                                    div { class: "px-3 pb-3 space-y-3",
                                        div {
                                            div { class: "text-xs text-fg-faint mb-1", "parameters" }
                                            pre { class: "font-mono text-xs bg-surface-3 rounded p-2 overflow-x-auto",
                                                {serde_json::to_string_pretty(&row.params).unwrap_or_default()}
                                            }
                                        }
                                        if let Some(report) = &row.report {
                                            RunReportView { report: report.clone() }
                                        }
                                        if !row.log.is_empty() {
                                            details {
                                                summary { class: "cursor-pointer select-none text-xs text-fg-muted", "debug log" }
                                                div { class: "mt-1 max-h-60 overflow-y-auto font-mono text-xs space-y-0.5",
                                                    for event in row.log.iter() {
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
                        }
                    },
                    Some(Err(e)) => rsx! { div { class: "text-sm text-danger", "failed to load history: {e}" } },
                    None => rsx! { div { class: "text-sm text-fg-muted", "loading…" } },
                }
            }
        }
    }
}
