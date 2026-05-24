use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};
#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrgCertDisplay {
    pub id: uuid::Uuid,
    pub fingerprint: String,
    pub label: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn list_org_certs(organization_id: String) -> Result<Vec<OrgCertDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = organization_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if !user.is_admin && !user.org_ids().contains(&oid) {
        return Err(ServerFnError::new("access denied"));
    }
    let certs = sqlx::query_as::<_, OrgCertDisplay>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'organization' AND scope_id = $1 AND is_ca = false \
         ORDER BY created_at",
    )
    .bind(oid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(certs)
}

#[server]
async fn add_org_cert(
    organization_id: String,
    fingerprint: String,
    certificate_pem: String,
    label: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = organization_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;

    let lbl = label.trim().to_string();
    let (fp, pem) = if !certificate_pem.trim().is_empty() {
        let fp = crate::api::routes::fingerprint_from_pem_str(&certificate_pem)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        (fp, Some(certificate_pem))
    } else if !fingerprint.trim().is_empty() {
        (fingerprint.trim().to_lowercase(), None)
    } else {
        return Err(ServerFnError::new("fingerprint or certificate PEM required"));
    };

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('organization', $1, false, $2, $3, $4)",
    )
    .bind(oid)
    .bind(&fp)
    .bind(&pem)
    .bind(&lbl)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_org_cert(organization_id: String, cert_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = organization_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cert_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "DELETE FROM client_certificates \
         WHERE id = $1 AND scope = 'organization' AND scope_id = $2 AND is_ca = false",
    )
    .bind(uuid)
    .bind(oid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn OrganizationClientCerts(organization_id: String, read_only: bool) -> Element {
    let oid_list = organization_id.clone();
    let mut certs = use_server_future(move || {
        let oid = oid_list.clone();
        async move { list_org_certs(oid).await }
    })?;

    let mut fp_input = use_signal(String::new);
    let mut pem_input = use_signal(String::new);
    let mut label_input = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    let oid_add = organization_id.clone();

    rsx! {
        if !read_only {
            if let Some(err) = &*error_msg.read() {
                ErrorText { class: "mb-2", "{err}" }
            }
            form { class: "flex flex-col gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let oid = oid_add.clone();
                    let fp = fp_input.read().clone();
                    let pem = pem_input.read().clone();
                    let lbl = label_input.read().clone();
                    spawn(async move {
                        if !fp.trim().is_empty() || !pem.trim().is_empty() {
                            match add_org_cert(oid, fp, pem, lbl).await {
                                Ok(()) => {
                                    error_msg.set(None);
                                    fp_input.set(String::new());
                                    pem_input.set(String::new());
                                    label_input.set(String::new());
                                    certs.restart();
                                }
                                Err(e) => error_msg.set(Some(e.to_string())),
                            }
                        }
                    });
                },
                div { class: "flex gap-2",
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
                textarea { class: "input w-full py-1 text-xs font-mono h-20",
                    placeholder: "{t!(\"client-certs-pem-placeholder\")}",
                    value: "{pem_input}",
                    oninput: move |e| pem_input.set(e.value()),
                }
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
                            let oid = organization_id.clone();
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
                                    if !read_only {
                                        button { class: "link-danger text-sm",
                                            onclick: move |_| {
                                                let cid = cid.clone();
                                                let oid = oid.clone();
                                                spawn(async move {
                                                    if remove_org_cert(oid, cid).await.is_ok() {
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
                }
            },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}
