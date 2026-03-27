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

#[component]
pub fn CustomerDetail(id: String) -> Element {
    let id_clone = id.clone();
    let customer = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_customer(id).await }
    })?;

    match &*customer.read() {
        Some(Ok(c)) => {
            let created = c.created_at.format("%Y-%m-%d %H:%M").to_string();
            let cid = c.id.to_string();
            rsx! {
                h2 { class: "text-2xl font-bold mb-2", "{c.name}" }
                p { class: "text-gray-500 mb-6", "Created: {created}" }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Tokens" }
                        TokenList { customer_id: cid.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Config" }
                        ConfigEditor { customer_id: cid.clone() }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}
