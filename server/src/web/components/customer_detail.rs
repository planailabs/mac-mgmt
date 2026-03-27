use dioxus::prelude::*;

use crate::models::Customer;

use super::config_editor::ConfigEditor;
use super::token_list::TokenList;

#[server]
async fn get_customer(id: String) -> Result<Customer, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let customer = sqlx::query_as::<_, Customer>("SELECT * FROM customers WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(customer)
}

#[server]
async fn rename_customer(id: String, name: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE customers SET name = $1 WHERE id = $2")
        .bind(&name)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn CustomerDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut customer = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_customer(id).await }
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);

    match &*customer.read() {
        Some(Ok(c)) => {
            let created = c.created_at.format("%Y-%m-%d %H:%M").to_string();
            let cid = c.id.to_string();
            let cid2 = cid.clone();
            let name = c.name.clone();
            rsx! {
                div { class: "flex items-center gap-3 mb-2",
                    if *editing.read() {
                        form {
                            class: "flex items-center gap-2",
                            onsubmit: move |_| {
                                let id = cid.clone();
                                let new_name = draft_name.read().clone();
                                async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = rename_customer(id, new_name).await;
                                        customer.restart();
                                    }
                                    editing.set(false);
                                }
                            },
                            input {
                                class: "text-2xl font-bold border border-gray-300 rounded px-2 py-1",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            button {
                                class: "text-green-600 hover:text-green-800",
                                r#type: "submit",
                                "Save"
                            }
                            button {
                                class: "text-gray-500 hover:text-gray-700",
                                r#type: "button",
                                onclick: move |_| editing.set(false),
                                "Cancel"
                            }
                        }
                    } else {
                        h2 { class: "text-2xl font-bold", "{name}" }
                        button {
                            class: "text-gray-400 hover:text-gray-600",
                            onclick: move |_| {
                                draft_name.set(name.clone());
                                editing.set(true);
                            },
                            "Edit"
                        }
                    }
                }
                p { class: "text-gray-500 mb-6", "Created: {created}" }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Tokens" }
                        TokenList { customer_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Config" }
                        ConfigEditor { customer_id: cid2.clone() }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}
