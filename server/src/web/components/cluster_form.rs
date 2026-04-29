use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Cluster;
use crate::web::app::Route;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, PageHeader};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[server]
async fn create_cluster(name: String) -> Result<Cluster, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let cluster =
        sqlx::query_as::<_, Cluster>("INSERT INTO clusters (name) VALUES ($1) RETURNING *")
            .bind(&name)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(cluster)
}

#[component]
pub fn ClusterForm() -> Element {
    let navigator = navigator();
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator;
        let name_val = name.read().clone();
        spawn(async move {
            match create_cluster(name_val).await {
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
