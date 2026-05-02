use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Badge, BadgeVariant, ErrorText, HelpText, PageHeader, Th};

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
    use_topbar(t!("skill-center-list-title"), None);
    let skill_centers = use_server_future(list_skill_centers)?;

    rsx! {
        div { class: "px-6 py-8 max-w-5xl mx-auto",
            div { class: "flex items-center justify-between mb-6",
                PageHeader { class: "mb-0", {t!("skill-center-list-title")} }
                Link { to: crate::web::app::Route::SkillCenterForm {},
                    class: "btn btn-md btn-primary",
                    {t!("skill-center-list-new")}
                }
            }
            match &*skill_centers.read() {
                Some(Ok(centers)) => rsx! {
                    if centers.is_empty() {
                        HelpText { {t!("skill-center-list-none")} }
                    } else {
                        div { class: "overflow-x-auto",
                            table { class: "table",
                                thead { class: "thead",
                                    tr {
                                        Th { {t!("name")} }
                                        Th { {t!("skill-center-list-col-url")} }
                                        Th { {t!("skill-center-list-col-priority")} }
                                        Th { {t!("skill-center-list-col-enabled")} }
                                    }
                                }
                                tbody { class: "tbody",
                                    for center in centers {
                                        tr {
                                            td { class: "td",
                                                Link {
                                                    to: crate::web::app::Route::SkillCenterDetail { id: center.id.to_string() },
                                                    class: "link",
                                                    "{center.name}"
                                                }
                                            }
                                            td { class: "td-muted", "{center.url}" }
                                            td { class: "td", "{center.priority}" }
                                            td { class: "td",
                                                if center.enabled {
                                                    Badge { variant: BadgeVariant::Success, {t!("yes")} }
                                                } else {
                                                    Badge { variant: BadgeVariant::Danger, {t!("no")} }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { HelpText { {t!("loading")} } },
            }
        }
    }
}
