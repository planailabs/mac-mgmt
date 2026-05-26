use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct OrgCaDisplay {
    pub id: uuid::Uuid,
    pub fingerprint: String,
    pub label: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn list_org_cas(organization_id: String) -> Result<Vec<OrgCaDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let oid: uuid::Uuid = organization_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if !user.is_admin && !user.org_ids().contains(&oid) {
        return Err(ServerFnError::new("access denied"));
    }
    let cas = sqlx::query_as::<_, OrgCaDisplay>(
        "SELECT id, fingerprint, label, created_at \
         FROM client_certificates \
         WHERE scope = 'organization' AND scope_id = $1 AND is_ca = true \
         ORDER BY created_at",
    )
    .bind(oid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(cas)
}

#[server]
async fn add_org_ca(
    organization_id: String,
    certificate_pem: String,
    label: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = organization_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;

    let fp = crate::api::routes::fingerprint_from_pem_str(&certificate_pem)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let lbl = label.trim().to_string();

    sqlx::query(
        "INSERT INTO client_certificates (scope, scope_id, is_ca, fingerprint, certificate_pem, label) \
         VALUES ('organization', $1, true, $2, $3, $4)",
    )
    .bind(oid)
    .bind(&fp)
    .bind(&certificate_pem)
    .bind(&lbl)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_org_ca(organization_id: String, ca_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let oid: uuid::Uuid = organization_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_org_admin(&oid)?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = ca_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "DELETE FROM client_certificates \
         WHERE id = $1 AND scope = 'organization' AND scope_id = $2 AND is_ca = true",
    )
    .bind(uuid)
    .bind(oid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn OrganizationClientCas(organization_id: String, read_only: bool) -> Element {
    let oid_list = organization_id.clone();
    let mut cas = use_server_future(move || {
        let oid = oid_list.clone();
        async move { list_org_cas(oid).await }
    })?;

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
                    let pem = pem_input.read().clone();
                    let lbl = label_input.read().clone();
                    spawn(async move {
                        if !pem.trim().is_empty() {
                            match add_org_ca(oid, pem, lbl).await {
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
                textarea { class: "input w-full py-1 text-xs font-mono h-20",
                    placeholder: "{t!(\"client-cas-pem-placeholder\")}",
                    value: "{pem_input}",
                    oninput: move |e| pem_input.set(e.value()),
                }
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
                            let oid = organization_id.clone();
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
                                    if !read_only {
                                        button { class: "link-danger text-sm",
                                            onclick: move |_| {
                                                let cid = cid.clone();
                                                let oid = oid.clone();
                                                spawn(async move {
                                                    if remove_org_ca(oid, cid).await.is_ok() {
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
                }
            },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}
