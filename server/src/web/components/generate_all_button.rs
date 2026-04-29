use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use dioxus::prelude::*;
use dioxus_i18n::t;

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
pub fn GenerateAllButton(items: Vec<GenerateAllItem>, on_complete: EventHandler<()>) -> Element {
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
                class: "btn btn-xs btn-accent",
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
                    {t!("generate-all-generating")}
                } else if pending_count == 0 {
                    {t!("generate-all-done")}
                } else {
                    {t!("generate-all-pending", count: pending_count)}
                }
            }
            if is_running {
                div { class: "w-48 mt-1",
                    div { class: "flex justify-between text-xs text-fg-muted mb-1",
                        span { {t!("generate-all-progress", done: done_val, total: total_val)} }
                        if let Some(slug) = &*current_slug.read() {
                            span { class: "truncate ml-1 text-accent", "{slug}" }
                        }
                    }
                    div { class: "w-full bg-line-soft rounded h-3 overflow-hidden",
                        div {
                            class: "bg-accent h-3 rounded transition-all duration-300 ease-in-out",
                            style: "width: {pct}%; min-width: {min_w}",
                        }
                    }
                }
            }
            if let Some(err) = &*error_msg.read() {
                span { class: "text-xs text-danger max-w-xs truncate", "{err}" }
            }
        }
    }
}
