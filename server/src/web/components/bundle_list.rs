use dioxus::prelude::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::models::Bundle;
use crate::web::app::Route;
use crate::web::components::generate_all_button::GenerateAllButton;

#[server]
async fn list_bundles() -> Result<Vec<Bundle>, ServerFnError> {
    let pool = crate::server_pool()?;
    let bundles = sqlx::query_as::<_, Bundle>("SELECT * FROM bundles ORDER BY slug")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[component]
pub fn BundleList() -> Element {
    let mut bundles = use_server_future(list_bundles)?;

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Bundles" }
            div { class: "flex items-center gap-2",
                {match &*bundles.read() {
                    Some(Ok(list)) => {
                        let gen_items: Vec<GenerateAllItem> = list.iter().map(|b| GenerateAllItem {
                            id: b.id.to_string(),
                            name: b.name.clone(),
                            description: b.description.clone(),
                            context: GenerateContext::Bundle { slug: b.slug.clone(), items: vec![] },
                            entity_kind: EntityKind::Bundle,
                        }).collect();
                        rsx! {
                            GenerateAllButton {
                                items: gen_items,
                                on_complete: move |_| { bundles.restart(); },
                            }
                        }
                    },
                    _ => rsx! {},
                }}
                Link {
                    to: Route::BundleForm {},
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    "New Bundle"
                }
            }
        }
        {match &*bundles.read() {
            Some(Ok(list)) => rsx! {
                table { class: "min-w-full divide-y divide-gray-200",
                    thead { class: "bg-gray-50",
                        tr {
                            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Slug" }
                            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Name" }
                            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Created" }
                        }
                    }
                    tbody { class: "bg-white divide-y divide-gray-200",
                        for bundle in list {
                            {
                                let bid = bundle.id.to_string();
                                let slug = bundle.slug.clone();
                                let name = bundle.name.clone();
                                let created = bundle.created_at.format("%Y-%m-%d %H:%M").to_string();
                                rsx! {
                                    tr { key: "{bid}",
                                        td { class: "px-6 py-4",
                                            Link {
                                                to: Route::BundleDetail { id: bid },
                                                class: "text-blue-600 hover:underline font-mono text-sm",
                                                "{slug}"
                                            }
                                        }
                                        td { class: "px-6 py-4", "{name}" }
                                        td { class: "px-6 py-4 text-gray-500", "{created}" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}
    }
}
