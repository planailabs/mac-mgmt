use dioxus::prelude::*;

use dioxus_i18n::t;

use crate::api_mcp::endpoints::users::{
    UserListInput, UserRow, UserSetAdminInput, list_users, toggle_user_admin,
};
use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{DataTable, ErrorText, PageHeader, Td, TdMuted, Th, page_window};

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

#[component]
pub fn UserList() -> Element {
    use_topbar(t!("user-list-title"), None);
    let users_future = use_server_future(|| list_users(UserListInput {}))?;

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
fn UserTable(
    list: Vec<UserRow>,
    users_future: Resource<Result<Vec<UserRow>, ServerFnError>>,
) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let page = use_signal(|| 0usize);

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone
                .iter()
                .filter(|u| u.matches_search(&q))
                .cloned()
                .collect()
        }
    });

    let total = list.len();
    let filtered_count = filtered.read().len();
    let limit_val = *limit.read();
    let (start, shown) = page_window(*page.read(), limit_val, filtered_count);

    rsx! {
        DataTable {
            search, limit, page, total, filtered: filtered_count, shown,
            headers: rsx! {
                Th { {t!("email")} }
                Th { {t!("name")} }
                Th { {t!("admin")} }
                Th { {t!("user-list-col-orgs")} }
                Th { {t!("created")} }
            },
            body: rsx! {
                for user in filtered.read().iter().skip(start).take(limit_val) {
                    UserRowView { key: "{user.id}", user: user.clone(), users_future }
                }
            },
        }
    }
}

#[component]
fn UserRowView(
    user: UserRow,
    users_future: Resource<Result<Vec<UserRow>, ServerFnError>>,
) -> Element {
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
                            if let Ok(id) = uid.parse::<uuid::Uuid>() {
                                let _ = toggle_user_admin(UserSetAdminInput {
                                    id,
                                    is_admin: new_val,
                                })
                                .await;
                                users_future.restart();
                            }
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
