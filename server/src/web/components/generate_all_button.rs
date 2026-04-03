use dioxus::prelude::*;

use crate::anthropic::{
    GenerateAllItem, generate_name_desc, save_generated_name_desc,
};

#[component]
pub fn GenerateAllButton(
    items: Vec<GenerateAllItem>,
    on_complete: EventHandler<()>,
) -> Element {
    let mut running = use_signal(|| false);
    let mut done = use_signal(|| 0usize);
    let mut total = use_signal(|| 0usize);
    let mut current_slug = use_signal(|| None::<String>);
    let mut error_msg = use_signal(|| None::<String>);

    let needs_gen: Vec<GenerateAllItem> = items
        .iter()
        .filter(|i| i.description.trim().is_empty())
        .cloned()
        .collect();
    let pending_count = needs_gen.len();

    rsx! {
        div { class: "inline-flex flex-col gap-1",
            button {
                class: "text-xs bg-purple-100 text-purple-700 px-2 py-1 rounded hover:bg-purple-200 disabled:opacity-50",
                r#type: "button",
                disabled: *running.read() || pending_count == 0,
                onclick: move |_| {
                    let batch = needs_gen.clone();
                    spawn(async move {
                        running.set(true);
                        error_msg.set(None);
                        total.set(batch.len());
                        done.set(0);

                        for item in &batch {
                            current_slug.set(Some(
                                match &item.context {
                                    crate::anthropic::GenerateContext::Skill { .. } => item.name.clone(),
                                    crate::anthropic::GenerateContext::Bundle { slug, .. } => slug.clone(),
                                    crate::anthropic::GenerateContext::McpServer { slug, .. } => slug.clone(),
                                    crate::anthropic::GenerateContext::McpBundle { slug, .. } => slug.clone(),
                                }
                            ));

                            match generate_name_desc(
                                item.context.clone(),
                                item.name.clone(),
                                item.description.clone(),
                            ).await {
                                Ok(result) => {
                                    if let Err(e) = save_generated_name_desc(
                                        item.entity_kind.clone(),
                                        item.id.clone(),
                                        result.name,
                                        result.description,
                                    ).await {
                                        error_msg.set(Some(format!("Save failed: {e}")));
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error_msg.set(Some(format!("Generate failed: {e}")));
                                    break;
                                }
                            }
                            done.set(done() + 1);
                        }

                        current_slug.set(None);
                        running.set(false);
                        on_complete.call(());
                    });
                },
                if *running.read() {
                    "Generating..."
                } else if pending_count == 0 {
                    "All have descriptions"
                } else {
                    "Generate all ({pending_count})"
                }
            }
            if *running.read() {
                div { class: "w-48",
                    div { class: "flex justify-between text-xs text-gray-500 mb-0.5",
                        span { "{done}/{total}" }
                        if let Some(slug) = &*current_slug.read() {
                            span { class: "truncate ml-1", "{slug}" }
                        }
                    }
                    {
                        let pct = if *total.read() > 0 {
                            (*done.read() as f64 / *total.read() as f64 * 100.0) as u32
                        } else {
                            0
                        };
                        rsx! {
                            div { class: "w-full bg-gray-200 rounded-full h-2",
                                div {
                                    class: "bg-purple-600 h-2 rounded-full transition-all duration-300",
                                    style: "width: {pct}%",
                                }
                            }
                        }
                    }
                }
            }
            if let Some(err) = &*error_msg.read() {
                span { class: "text-xs text-red-600 max-w-xs truncate", "{err}" }
            }
        }
    }
}
