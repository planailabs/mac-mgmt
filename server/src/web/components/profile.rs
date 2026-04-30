use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::ui::{Badge, BadgeVariant, ErrorText};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileInfo {
    name: String,
    email: String,
    is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileOrg {
    id: String,
    name: String,
    role: String,
}

#[server]
async fn get_profile() -> Result<ProfileInfo, ServerFnError> {
    let user = current_user().await?;
    Ok(ProfileInfo {
        name: user.name,
        email: user.email,
        is_admin: user.is_admin,
    })
}

#[server]
async fn get_profile_orgs() -> Result<Vec<ProfileOrg>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        name: String,
        role: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT o.id, o.name, om.role \
         FROM organization_members om \
         JOIN organizations o ON o.id = om.organization_id \
         WHERE om.user_id = $1 \
         ORDER BY o.name",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ProfileOrg {
            id: r.id.to_string(),
            name: r.name,
            role: r.role,
        })
        .collect())
}

#[component]
pub fn Profile() -> Element {
    let profile_future = use_server_future(get_profile)?;
    let orgs_future = use_server_future(get_profile_orgs)?;

    match &*profile_future.read() {
        Some(Ok(info)) => {
            let orgs = match &*orgs_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            rsx! {
                div { class: "space-y-6",
                    // Header
                    div { class: "flex items-center gap-4",
                        // Large profile icon
                        div { class: "flex items-center justify-center h-16 w-16 rounded-full bg-surface-2 text-fg-muted",
                            svg {
                                class: "h-10 w-10",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "1.5",
                                view_box: "0 0 24 24",
                                path {
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    d: "M17.982 18.725A7.488 7.488 0 0 0 12 15.75a7.488 7.488 0 0 0-5.982 2.975m11.963 0a9 9 0 1 0-11.963 0m11.963 0A8.966 8.966 0 0 1 12 21a8.966 8.966 0 0 1-5.982-2.275M15 9.75a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
                                }
                            }
                        }
                        div {
                            h2 { class: "text-2xl font-bold text-fg-strong", "{info.name}" }
                            p { class: "text-fg-muted text-sm", "{info.email}" }
                        }
                    }

                    // Details card
                    div { class: "card divide-y divide-line-soft",
                        div { class: "px-4 py-3 flex justify-between items-center",
                            span { class: "text-sm font-medium text-fg-muted", {t!("name")} }
                            span { class: "text-sm text-fg-strong", "{info.name}" }
                        }
                        div { class: "px-4 py-3 flex justify-between items-center",
                            span { class: "text-sm font-medium text-fg-muted", {t!("email")} }
                            span { class: "text-sm text-fg-strong", "{info.email}" }
                        }
                        div { class: "px-4 py-3 flex justify-between items-center",
                            span { class: "text-sm font-medium text-fg-muted", {t!("profile-role")} }
                            if info.is_admin {
                                Badge { variant: BadgeVariant::Accent, {t!("admin")} }
                            } else {
                                Badge { variant: BadgeVariant::Neutral, {t!("member")} }
                            }
                        }
                    }

                    // Organizations card
                    div { class: "card p-4",
                        h3 { class: "h-section text-fg-strong", {t!("profile-organizations")} }
                        if orgs.is_empty() {
                            p { class: "text-fg-muted text-sm", {t!("profile-no-orgs")} }
                        } else {
                            div { class: "divide-y divide-line-soft",
                                for o in &orgs {
                                    {
                                        let badge_variant = match o.role.as_str() {
                                            "admin" => BadgeVariant::Accent,
                                            "write" => BadgeVariant::Info,
                                            _ => BadgeVariant::Neutral,
                                        };
                                        rsx! {
                                            div { class: "flex items-center justify-between py-2",
                                                Link {
                                                    to: Route::OrganizationDetail { id: o.id.clone() },
                                                    class: "link text-sm font-medium",
                                                    "{o.name}"
                                                }
                                                Badge { variant: badge_variant, "{o.role}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { p { {t!("loading")} } },
    }
}
