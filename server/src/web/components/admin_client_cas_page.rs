use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};

#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct AdminCaDisplay {
    pub id: uuid::Uuid,
    pub fingerprint: String,
    pub label: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn list_admin_cas() -> Result<Vec<AdminCaDisplay>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let cas = sqlx::query_as::<_, AdminCaDisplay>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'admin' AND is_ca = true \
         ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(cas)
}

#[server]
async fn add_admin_ca(certificate_pem: String, label: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let fp = crate::api::routes::fingerprint_from_pem_str(&certificate_pem)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let lbl = label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('admin', NULL, true, $1, $2, $3)",
    )
    .bind(&fp)
    .bind(&certificate_pem)
    .bind(&lbl)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_admin_ca(ca_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = ca_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "DELETE FROM client_certificates WHERE id = $1 AND scope = 'admin' AND is_ca = true",
    )
    .bind(uuid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn AdminClientCas() -> Element {
    use_topbar(t!("admin-client-cas-title"), None);

    let mut cas = use_server_future(|| async { list_admin_cas().await })?;

    let mut pem_input = use_signal(String::new);
    let mut label_input = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        h2 { class: "h-page text-fg-strong", {t!("admin-client-cas-title")} }
        p { class: "text-fg mb-6 text-sm",
            {t!("admin-client-cas-description")}
        }

        if let Some(err) = &*error_msg.read() {
            ErrorText { class: "mb-2", "{err}" }
        }

        form { class: "flex flex-col gap-2 mb-6",
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                let pem = pem_input.read().clone();
                let lbl = label_input.read().clone();
                spawn(async move {
                    if !pem.trim().is_empty() {
                        match add_admin_ca(pem, lbl).await {
                            Ok(()) => {
                                error_msg.set(None);
                                pem_input.set(String::new());
                                label_input.set(String::new());
                                cas.restart();
                            }
                            Err(e) => error_msg.set(Some(e.to_string())),
                        }
                    }
                });
            },
            div { class: "flex gap-2",
                input { class: "input flex-1 w-auto py-1 text-sm",
                    placeholder: "{t!(\"client-certs-label-placeholder\")}",
                    value: "{label_input}",
                    oninput: move |e| label_input.set(e.value()),
                }
                Button {
                    kind: ButtonKind::Submit,
                    size: ButtonSize::Sm,
                    class: "self-start",
                    {t!("add")}
                }
            }
            textarea { class: "input w-full py-1 text-xs font-mono h-24",
                placeholder: "{t!(\"client-cas-pem-placeholder\")}",
                value: "{pem_input}",
                oninput: move |e| pem_input.set(e.value()),
            }
        }

        {match &*cas.read() {
            Some(Ok(list)) if list.is_empty() => rsx! {
                HelpText { {t!("client-cas-no-cas")} }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-line-soft",
                    for ca in list {
                        {
                            let cid = ca.id.to_string();
                            let fp = ca.fingerprint.clone();
                            let label = ca.label.clone();
                            rsx! {
                                li { class: "py-2 flex justify-between items-center",
                                    div {
                                        span { class: "text-sm font-mono", "{fp}" }
                                        if !label.is_empty() {
                                            span { class: "text-xs text-fg-muted ml-2", "{label}" }
                                        }
                                    }
                                    button { class: "link-danger text-sm",
                                        onclick: move |_| {
                                            let cid = cid.clone();
                                            spawn(async move {
                                                if remove_admin_ca(cid).await.is_ok() {
                                                    cas.restart();
                                                }
                                            });
                                        },
                                        {t!("remove")}
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}
