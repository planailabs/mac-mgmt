use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use dioxus_i18n::t;

use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{DataTable, ErrorText, PageHeader, Td, TdMuted, Th};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserRow {
    id: String,
    email: String,
    name: String,
    is_admin: bool,
    org_names: Vec<String>,
    created_at: DateTime<Utc>,
}

impl Searchable for UserRow {
    fn matches_search(&self, query: &str) -> bool {
        self.email.to_lowercase().contains(query)
            || self.name.to_lowercase().contains(query)
            || self
                .org_names
                .iter()
                .any(|o| o.to_lowercase().contains(query))
    }
}

#[server]
async fn list_users() -> Result<Vec<UserRow>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        email: String,
        name: String,
        is_admin: bool,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, email, name, is_admin, created_at FROM users ORDER BY email",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut users = Vec::with_capacity(rows.len());
    for r in rows {
        let org_names: Vec<String> = sqlx::query_scalar(
            "SELECT o.name FROM organizations o \
             JOIN organization_members om ON om.organization_id = o.id \
             WHERE om.user_id = $1 \
             ORDER BY o.name",
        )
        .bind(r.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        users.push(UserRow {
            id: r.id.to_string(),
            email: r.email,
            name: r.name,
            is_admin: r.is_admin,
            org_names,
            created_at: r.created_at,
        });
    }

    Ok(users)
}

#[server]
async fn toggle_user_admin(user_id: String, is_admin: bool) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;

    let uid: uuid::Uuid = user_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    if uid == user.id && !is_admin {
        return Err(ServerFnError::new("cannot remove your own admin status"));
    }

    let pool = crate::server_pool()?;
    sqlx::query("UPDATE users SET is_admin = $2 WHERE id = $1")
        .bind(uid)
        .bind(is_admin)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn UserList() -> Element {
    use_topbar(t!("user-list-title"), None);
    let users_future = use_server_future(list_users)?;

    rsx! {
        div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4",
            PageHeader { class: "mb-0", {t!("user-list-title")} }
            Link { to: Route::UserForm {}, class: "btn btn-md btn-primary",
                {t!("user-list-new")}
            }
        }
        {match &*users_future.read() {
            Some(Ok(list)) => rsx! { UserTable { list: list.clone(), users_future } },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { p { {t!("loading")} } },
        }}
    }
}

#[component]
fn UserTable(list: Vec<UserRow>, users_future: Resource<Result<Vec<UserRow>, ServerFnError>>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone.iter().filter(|u| u.matches_search(&q)).cloned().collect()
        }
    });

    let total = list.len();
    let filtered_count = filtered.read().len();
    let limit_val = *limit.read();
    let shown = filtered_count.min(limit_val);

    rsx! {
        DataTable {
            search, limit, total, filtered: filtered_count, shown,
            headers: rsx! {
                Th { {t!("email")} }
                Th { {t!("name")} }
                Th { {t!("admin")} }
                Th { {t!("user-list-col-orgs")} }
                Th { {t!("created")} }
            },
            body: rsx! {
                for user in filtered.read().iter().take(limit_val) {
                    UserRowView { key: "{user.id}", user: user.clone(), users_future }
                }
            },
        }
    }
}

#[component]
fn UserRowView(user: UserRow, users_future: Resource<Result<Vec<UserRow>, ServerFnError>>) -> Element {
    let mut users_future = users_future;
    let uid = user.id.clone();
    let is_admin = user.is_admin;
    let orgs_display = user.org_names.join(", ");
    let created = user.created_at.format("%Y-%m-%d %H:%M").to_string();
    rsx! {
        tr {
            Td { class: "text-sm font-medium",
                Link { to: Route::UserDetail { id: user.id.clone() }, class: "link",
                    "{user.email}"
                }
            }
            Td { class: "text-sm", "{user.name}" }
            Td { class: "text-sm",
                input {
                    r#type: "checkbox",
                    checked: is_admin,
                    class: "h-4 w-4 rounded border-line text-brand focus:ring-brand",
                    onchange: move |e: Event<FormData>| {
                        let uid = uid.clone();
                        let new_val = e.checked();
                        async move {
                            let _ = toggle_user_admin(uid, new_val).await;
                            users_future.restart();
                        }
                    },
                }
            }
            Td { class: "text-sm",
                if orgs_display.is_empty() {
                    span { class: "text-fg-faint italic", {t!("none")} }
                } else {
                    "{orgs_display}"
                }
            }
            TdMuted { class: "text-sm", {created} }
        }
    }
}
