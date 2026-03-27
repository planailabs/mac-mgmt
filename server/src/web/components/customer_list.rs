use dioxus::prelude::*;

use crate::models::Customer;
use crate::web::app::Route;

#[server]
async fn list_customers() -> Result<Vec<Customer>, ServerFnError> {
    let pool = crate::server_pool()?;
    let customers = sqlx::query_as::<_, Customer>("SELECT * FROM customers ORDER BY name")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(customers)
}

#[component]
pub fn CustomerList() -> Element {
    let customers = use_server_future(list_customers)?;

    match &*customers.read() {
        Some(Ok(list)) => rsx! {
            h2 { class: "text-2xl font-bold mb-4", "Customers" }
            table { class: "min-w-full divide-y divide-gray-200",
                thead { class: "bg-gray-50",
                    tr {
                        th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Name" }
                        th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Created" }
                    }
                }
                tbody { class: "bg-white divide-y divide-gray-200",
                    for customer in list {
                        tr {
                            td { class: "px-6 py-4",
                                Link { to: Route::CustomerDetail { id: customer.id.to_string() },
                                    class: "text-blue-600 hover:underline",
                                    "{customer.name}"
                                }
                            }
                            td { class: "px-6 py-4 text-gray-500",
                                { customer.created_at.format("%Y-%m-%d %H:%M").to_string() }
                            }
                        }
                    }
                }
            }
        },
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}
