use dioxus::prelude::*;

use crate::models::Skill;
use crate::web::app::Route;

#[server]
async fn list_skills() -> Result<Vec<Skill>, ServerFnError> {
    let pool = crate::server_pool()?;
    let skills = sqlx::query_as::<_, Skill>("SELECT * FROM skills ORDER BY slug")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(skills)
}

#[component]
pub fn SkillList() -> Element {
    let skills = use_server_future(list_skills)?;

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Skills" }
            Link {
                to: Route::SkillForm {},
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                "New Skill"
            }
        }
        {match &*skills.read() {
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
                        for skill in list {
                            {
                                let sid = skill.id.to_string();
                                let slug = skill.slug.clone();
                                let name = skill.name.clone();
                                let created = skill.created_at.format("%Y-%m-%d %H:%M").to_string();
                                rsx! {
                                    tr { key: "{sid}",
                                        td { class: "px-6 py-4",
                                            Link {
                                                to: Route::SkillDetail { id: sid },
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
