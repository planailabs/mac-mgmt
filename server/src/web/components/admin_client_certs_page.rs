use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};

#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct AdminCertDisplay {
    pub id: uuid::Uuid,
    pub fingerprint: String,
    pub label: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn list_admin_certs() -> Result<Vec<AdminCertDisplay>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let certs = sqlx::query_as::<_, AdminCertDisplay>(
        "SELECT id, fingerprint, label, created_at \
         FROM admin_client_certs ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(certs)
}

#[server]
async fn add_admin_cert(fingerprint: String, label: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let fp = fingerprint.trim().to_lowercase();
    let lbl = label.trim().to_string();

    sqlx::query("INSERT INTO admin_client_certs (fingerprint, label) VALUES ($1, $2)")
        .bind(&fp)
        .bind(&lbl)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_admin_cert(cert_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cert_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM admin_client_certs WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn AdminClientCerts() -> Element {
    use_topbar(t!("admin-client-certs-title"), None);

    let mut certs = use_server_future(|| async { list_admin_certs().await })?;

    let mut fp_input = use_signal(String::new);
    let mut label_input = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    rsx! {
        h2 { class: "h-page text-fg-strong", {t!("admin-client-certs-title")} }
        p { class: "text-fg mb-6 text-sm",
            {t!("admin-client-certs-description")}
        }

        if let Some(err) = &*error_msg.read() {
            ErrorText { class: "mb-2", "{err}" }
        }

        form { class: "flex gap-2 mb-6",
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                let fp = fp_input.read().clone();
                let lbl = label_input.read().clone();
                spawn(async move {
                    if !fp.trim().is_empty() {
                        match add_admin_cert(fp, lbl).await {
                            Ok(()) => {
                                error_msg.set(None);
                                fp_input.set(String::new());
                                label_input.set(String::new());
                                certs.restart();
                            }
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                            }
                        }
                    }
                });
            },
            input { class: "input flex-1 w-auto py-1 text-sm font-mono",
                placeholder: "{t!(\"client-certs-fingerprint-placeholder\")}",
                value: "{fp_input}",
                oninput: move |e| fp_input.set(e.value()),
            }
            input { class: "input w-48 py-1 text-sm",
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

        {match &*certs.read() {
            Some(Ok(list)) if list.is_empty() => rsx! {
                HelpText { {t!("client-certs-no-certs")} }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-line-soft",
                    for cert in list {
                        {
                            let cid = cert.id.to_string();
                            let fp = cert.fingerprint.clone();
                            let label = cert.label.clone();
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
                                                if remove_admin_cert(cid).await.is_ok() {
                                                    certs.restart();
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
