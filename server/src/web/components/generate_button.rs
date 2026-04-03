use dioxus::prelude::*;

use crate::anthropic::{GenerateContext, generate_name_desc};

#[component]
pub fn GenerateButton(
    context: GenerateContext,
    name_signal: Signal<String>,
    desc_signal: Signal<String>,
) -> Element {
    let mut generating = use_signal(|| false);
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        span { class: "inline-flex items-center gap-1",
            button {
                class: "text-xs bg-purple-100 text-purple-700 px-2 py-1 rounded hover:bg-purple-200 disabled:opacity-50",
                r#type: "button",
                disabled: *generating.read(),
                onclick: move |_| {
                    let ctx = context.clone();
                    let current_name = name_signal.read().clone();
                    let current_desc = desc_signal.read().clone();
                    spawn(async move {
                        generating.set(true);
                        error_msg.set(None);
                        match generate_name_desc(ctx, current_name, current_desc).await {
                            Ok(result) => {
                                name_signal.set(result.name);
                                desc_signal.set(result.description);
                            }
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                            }
                        }
                        generating.set(false);
                    });
                },
                if *generating.read() { "Generating..." } else { "Generate with AI" }
            }
            if let Some(err) = &*error_msg.read() {
                span { class: "text-xs text-red-600", "{err}" }
            }
        }
    }
}
