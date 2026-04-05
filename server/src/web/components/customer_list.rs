use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::models::Customer;
use crate::web::app::Route;
use crate::web::components::table_utils::*;

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

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Customers" }
            Link {
                to: Route::CustomerForm {},
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                "New Customer"
            }
        }
        {match &*customers.read() {
            Some(Ok(list)) => {
                let search = use_signal(String::new);
                let limit = use_signal(|| 20usize);

                let list_clone = list.clone();
                let filtered = use_memo(move || {
                    let q = search.read().to_lowercase();
                    if q.is_empty() {
                        list_clone.clone()
                    } else {
                        list_clone.iter().filter(|c| c.matches_search(&q)).cloned().collect()
                    }
                });

                let total = list.len();
                let data = use_tabular(
                    (LinkColumn { header: "Name" }, CreatedAtColumn),
                    filtered.into(),
                );
                let all_rows: Vec<_> = data.rows().collect();
                let filtered_count = all_rows.len();
                let limit_val = *limit.read();
                let shown = filtered_count.min(limit_val);

                rsx! {
                    TableToolbar { search, limit, total, filtered: filtered_count, shown }
                    table { class: "min-w-full divide-y divide-gray-200",
                        thead { class: "bg-gray-50",
                            tr { TableHeaders { data } }
                        }
                        tbody { class: "bg-white divide-y divide-gray-200",
                            for row in all_rows.into_iter().take(limit_val) {
                                tr { key: "{row.key()}", TableCells { row } }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}
    }
}
