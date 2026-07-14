use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::clusters::{ClusterCreateInput, create_cluster};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, PageHeader};

#[component]
pub fn ClusterForm() -> Element {
    use_topbar(t!("nav-clusters"), None);
    let navigator = navigator();
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator;
        let name_val = name.read().clone();
        spawn(async move {
            match create_cluster(ClusterCreateInput { name: name_val }).await {
                Ok(cluster) => {
                    nav.push(Route::ClusterDetail {
                        id: cluster.id.to_string(),
                    });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        PageHeader { {t!("cluster-form-title")} }
        if let Some(err) = &*error.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            FormField { label: t!("name"),
                input {
                    class: "input",
                    r#type: "text",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }
            Button { kind: ButtonKind::Submit, {t!("create")} }
        }
    }
}
