use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::anthropic::{GenerateContext, GeneratedNameDesc, generate_name_desc};

#[component]
pub fn GenerateButton(
    context: GenerateContext,
    current_name: String,
    current_desc: String,
    on_generated: EventHandler<GeneratedNameDesc>,
) -> Element {
    let mut generating = use_signal(|| false);
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        span { class: "inline-flex items-center gap-1",
            button {
                class: "text-xs bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-200 px-2 py-1 rounded hover:bg-purple-200 dark:hover:bg-purple-800 disabled:opacity-50",
                r#type: "button",
                disabled: *generating.read(),
                onclick: move |evt| {
                    evt.prevent_default();
                    evt.stop_propagation();
                    let ctx = context.clone();
                    let cn = current_name.clone();
                    let cd = current_desc.clone();
                    spawn(async move {
                        generating.set(true);
                        error_msg.set(None);
                        match generate_name_desc(ctx, cn, cd).await {
                            Ok(result) => {
                                on_generated.call(result);
                            }
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                            }
                        }
                        generating.set(false);
                    });
                },
                if *generating.read() { {t!("generate-generating")} } else { {t!("generate-with-ai")} }
            }
            if let Some(err) = &*error_msg.read() {
                span { class: "text-xs text-red-600 dark:text-red-400", "{err}" }
            }
        }
    }
}
