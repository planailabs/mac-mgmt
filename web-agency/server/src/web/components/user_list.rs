use dioxus::prelude::*;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, PageHeader, Td, TdMuted, Th,
};
use crate::web::app::Route;

// UserRow + the list endpoint now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::users::{UserListInput, list_users};

#[component]
pub fn UserList() -> Element {
    let users = use_server_future(move || list_users(UserListInput::default()))?;
    let rows = match &*users.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4",
            PageHeader { class: "mb-0", "Users" }
            Link { to: Route::UserForm {}, class: "btn btn-md btn-primary", "New User" }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Email" } Th { "Name" } Th { "Admin" } Th { "Joined" } Th { "" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "5", "No users" } }
                        }
                        for row in &rows {
                            {
                                let uid = row.id;
                                let id_str = row.id.to_string();
                                rsx! {
                                    tr {
                                        Td {
                                            Link { to: Route::UserDetail { id: id_str }, class: "link text-sm font-medium",
                                                "{row.email}"
                                            }
                                        }
                                        Td { "{row.name}" }
                                        Td {
                                            if row.is_admin {
                                                Badge { variant: BadgeVariant::Accent, "Admin" }
                                            } else {
                                                span { class: "text-fg-muted", "No" }
                                            }
                                        }
                                        TdMuted { "{row.created_at}" }
                                        Td {
                                            Button {
                                                variant: ButtonVariant::Warn,
                                                size: ButtonSize::Sm,
                                                onclick: move |_| {
                                                    let js = format!(
                                                        "document.cookie = 'impersonate_user_id={uid}; Path=/; SameSite=Lax'; window.location.href = '/';"
                                                    );
                                                    document::eval(&js);
                                                },
                                                "Impersonate"
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
    }
}
