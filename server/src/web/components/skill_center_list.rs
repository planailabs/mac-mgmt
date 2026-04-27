use dioxus::prelude::*;
use dioxus_i18n::t;

#[server]
async fn list_skill_centers() -> Result<Vec<SkillCenterRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, SkillCenterRow>(
        "SELECT id, name, url, priority, enabled, created_at, updated_at \
         FROM skill_centers ORDER BY priority DESC, name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("query failed: {e}")))?;

    Ok(rows)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillCenterRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub url: String,
    pub priority: i32,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[component]
pub fn SkillCenterList() -> Element {
    let skill_centers = use_server_future(list_skill_centers)?;

    rsx! {
        div { class: "px-6 py-8 max-w-5xl mx-auto",
            div { class: "flex items-center justify-between mb-6",
                h1 { class: "text-2xl font-bold dark:text-white", {t!("skill-center-list-title")} }
                Link {
                    to: crate::web::app::Route::SkillCenterForm {},
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    {t!("skill-center-list-new")}
                }
            }
            match &*skill_centers.read() {
                Some(Ok(centers)) => rsx! {
                    if centers.is_empty() {
                        p { class: "text-gray-500 dark:text-gray-400", {t!("skill-center-list-none")} }
                    } else {
                        table { class: "w-full text-sm text-left",
                            thead {
                                tr { class: "border-b dark:border-gray-700",
                                    th { class: "py-2 px-3 font-medium dark:text-gray-300", {t!("name")} }
                                    th { class: "py-2 px-3 font-medium dark:text-gray-300", {t!("skill-center-list-col-url")} }
                                    th { class: "py-2 px-3 font-medium dark:text-gray-300", {t!("skill-center-list-col-priority")} }
                                    th { class: "py-2 px-3 font-medium dark:text-gray-300", {t!("skill-center-list-col-enabled")} }
                                }
                            }
                            tbody {
                                for center in centers {
                                    tr { class: "border-b dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800",
                                        td { class: "py-2 px-3",
                                            Link {
                                                to: crate::web::app::Route::SkillCenterDetail { id: center.id.to_string() },
                                                class: "text-blue-600 hover:underline dark:text-blue-400",
                                                "{center.name}"
                                            }
                                        }
                                        td { class: "py-2 px-3 text-gray-600 dark:text-gray-400", "{center.url}" }
                                        td { class: "py-2 px-3 dark:text-gray-300", "{center.priority}" }
                                        td { class: "py-2 px-3",
                                            if center.enabled {
                                                span { class: "text-green-600 dark:text-green-400", {t!("yes")} }
                                            } else {
                                                span { class: "text-red-600 dark:text-red-400", {t!("no")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-500", {t!("error-message", message: e.to_string())} } },
                None => rsx! { p { class: "text-gray-500", {t!("loading")} } },
            }
        }
    }
}
