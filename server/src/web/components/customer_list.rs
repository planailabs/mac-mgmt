use dioxus::prelude::*;
use dioxus_tabular::*;

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

// Row impl

impl Row for Customer {
    fn key(&self) -> impl Into<String> {
        self.id.to_string()
    }
}

// Data accessors

#[derive(Clone, PartialEq)]
struct CustomerName {
    id: String,
    name: String,
}

#[derive(Clone, PartialEq)]
struct CustomerCreatedAt(String);

impl GetRowData<CustomerName> for Customer {
    fn get(&self) -> CustomerName {
        CustomerName {
            id: self.id.to_string(),
            name: self.name.clone(),
        }
    }
}

impl GetRowData<CustomerCreatedAt> for Customer {
    fn get(&self) -> CustomerCreatedAt {
        CustomerCreatedAt(self.created_at.format("%Y-%m-%d %H:%M").to_string())
    }
}

// Columns

#[derive(Clone, PartialEq)]
struct NameColumn;

impl<R: Row + GetRowData<CustomerName>> TableColumn<R> for NameColumn {
    fn column_name(&self) -> String {
        "name".into()
    }

    fn render_header(&self, _context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        rsx! {
            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase",
                "Name"
            }
        }
    }

    fn render_cell(&self, _context: ColumnContext, row: &R, _attributes: Vec<Attribute>) -> Element {
        let data: CustomerName = row.get();
        rsx! {
            td { class: "px-6 py-4",
                Link {
                    to: Route::CustomerDetail { id: data.id },
                    class: "text-blue-600 hover:underline",
                    "{data.name}"
                }
            }
        }
    }

    fn compare(&self, a: &R, b: &R) -> std::cmp::Ordering {
        let a: CustomerName = a.get();
        let b: CustomerName = b.get();
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    }
}

#[derive(Clone, PartialEq)]
struct CreatedAtColumn;

impl<R: Row + GetRowData<CustomerCreatedAt>> TableColumn<R> for CreatedAtColumn {
    fn column_name(&self) -> String {
        "created_at".into()
    }

    fn render_header(&self, _context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        rsx! {
            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase",
                "Created"
            }
        }
    }

    fn render_cell(&self, _context: ColumnContext, row: &R, _attributes: Vec<Attribute>) -> Element {
        let data: CustomerCreatedAt = row.get();
        rsx! {
            td { class: "px-6 py-4 text-gray-500",
                "{data.0}"
            }
        }
    }
}

// Component

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
                let rows = use_signal(|| list.clone());
                let data = use_tabular((NameColumn, CreatedAtColumn), rows.into());

                rsx! {
                    table { class: "min-w-full divide-y divide-gray-200",
                        thead { class: "bg-gray-50",
                            tr { TableHeaders { data } }
                        }
                        tbody { class: "bg-white divide-y divide-gray-200",
                            for row in data.rows() {
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
