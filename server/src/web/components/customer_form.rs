use dioxus::prelude::*;

use crate::models::Customer;
use crate::web::app::Route;

#[server]
async fn create_customer(name: String) -> Result<Customer, ServerFnError> {
    let pool = crate::server_pool()?;
    let customer = sqlx::query_as::<_, Customer>(
        "INSERT INTO customers (name) VALUES ($1) RETURNING *",
    )
    .bind(&name)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(customer)
}

#[component]
pub fn CustomerForm() -> Element {
    let navigator = navigator();
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator.clone();
        let name_val = name.read().clone();
        spawn(async move {
            match create_customer(name_val).await {
                Ok(customer) => {
                    nav.push(Route::CustomerDetail { id: customer.id.to_string() });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "New Customer" }
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 dark:text-red-400 mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Name" }
                input {
                    class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 dark:bg-gray-700 dark:text-white",
                    r#type: "text",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }
            button {
                class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                r#type: "submit",
                "Create"
            }
        }
    }
}
