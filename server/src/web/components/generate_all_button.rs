use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use dioxus::prelude::*;

use crate::anthropic::{GenerateAllItem, generate_name_desc, save_generated_name_desc};

/// A future that yields once to let the event loop process pending work.
struct YieldNow(bool);

fn yield_now() -> YieldNow {
    YieldNow(false)
}

impl Future for YieldNow {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

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

    // Read signals unconditionally so we're always subscribed to changes.
    let done_val = *done.read();
    let total_val = *total.read();
    let is_running = *running.read();
    let pct = if total_val > 0 {
        (done_val as f64 / total_val as f64 * 100.0) as u32
    } else {
        0
    };
    let min_w = if pct > 0 { "0.75rem" } else { "0" };

    rsx! {
        div { class: "inline-flex flex-col gap-1",
            button {
                class: "text-xs bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-200 px-2 py-1 rounded hover:bg-purple-200 dark:hover:bg-purple-800 disabled:opacity-50",
                r#type: "button",
                disabled: is_running || pending_count == 0,
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
                                    crate::anthropic::GenerateContext::Bundle { slug, .. }
                                    | crate::anthropic::GenerateContext::McpServer { slug, .. }
                                    | crate::anthropic::GenerateContext::McpBundle { slug, .. } => slug.clone(),
                                }
                            ));

                            // Yield to let the renderer paint the progress update.
                            yield_now().await;

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
                if is_running {
                    "Generating..."
                } else if pending_count == 0 {
                    "All have descriptions"
                } else {
                    "Generate all ({pending_count})"
                }
            }
            if is_running {
                div { class: "w-48 mt-1",
                    div { class: "flex justify-between text-xs text-gray-500 dark:text-gray-400 mb-1",
                        span { "{done_val} / {total_val}" }
                        if let Some(slug) = &*current_slug.read() {
                            span { class: "truncate ml-1 text-purple-600 dark:text-purple-400", "{slug}" }
                        }
                    }
                    div { class: "w-full bg-gray-200 dark:bg-gray-700 rounded h-3 overflow-hidden",
                        div {
                            class: "bg-purple-600 h-3 rounded transition-all duration-300 ease-in-out",
                            style: "width: {pct}%; min-width: {min_w}",
                        }
                    }
                }
            }
            if let Some(err) = &*error_msg.read() {
                span { class: "text-xs text-red-600 dark:text-red-400 max-w-xs truncate", "{err}" }
            }
        }
    }
}
