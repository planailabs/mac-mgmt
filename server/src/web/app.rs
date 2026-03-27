use dioxus::prelude::*;

use super::components::customer_detail::CustomerDetail;
use super::components::customer_form::CustomerForm;
use super::components::customer_list::CustomerList;
use super::components::layout::Layout;

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    CustomerList {},
    #[route("/customers/new")]
    CustomerForm {},
    #[route("/customers/:id")]
    CustomerDetail { id: String },
}

#[component]
pub fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: "/tailwind.css" }
        Router::<Route> {}
    }
}
