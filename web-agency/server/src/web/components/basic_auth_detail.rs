use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BasicAuthListData {
    id: Uuid,
    name: String,
    organization_name: String,
    credentials: Vec<CredentialRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialRow {
    id: Uuid,
    username: String,
    created_at: String,
}

#[server]
async fn get_basic_auth_list(list_id: Uuid) -> Result<BasicAuthListData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, Uuid)>(
        "SELECT id, name, organization_id FROM basic_auth_lists WHERE id = $1",
    )
    .bind(list_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("list not found"))?;

    let (id, name, org_id) = row;
    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let creds = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, username, created_at FROM basic_auth_credentials WHERE list_id = $1 ORDER BY username",
    )
    .bind(list_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(BasicAuthListData {
        id,
        name,
        organization_name: org_name,
        credentials: creds.into_iter().map(|(id, username, created_at)| CredentialRow {
            id, username, created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        }).collect(),
    })
}

#[server]
async fn add_credential(list_id: Uuid, username: String, password: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM basic_auth_lists WHERE id = $1",
    ).bind(list_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(password.as_bytes()));

    sqlx::query(
        "INSERT INTO basic_auth_credentials (list_id, username, password_hash) VALUES ($1, $2, $3) \
         ON CONFLICT (list_id, username) DO UPDATE SET password_hash = $3",
    )
    .bind(list_id).bind(&username).bind(&hash)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn remove_credential(credential_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT b.organization_id FROM basic_auth_credentials c \
         JOIN basic_auth_lists b ON b.id = c.list_id WHERE c.id = $1",
    ).bind(credential_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query("DELETE FROM basic_auth_credentials WHERE id = $1")
        .bind(credential_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn delete_basic_auth_list(list_id: Uuid) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM basic_auth_lists WHERE id = $1",
    ).bind(list_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // Clear references from webspaces
    sqlx::query("UPDATE webspaces SET auth_mode = 'none', auth_basic_list_id = NULL WHERE auth_basic_list_id = $1")
        .bind(list_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    sqlx::query("DELETE FROM basic_auth_lists WHERE id = $1")
        .bind(list_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::internal::notify_proxy_reload();
    Ok(())
}

#[component]
pub fn BasicAuthDetail(id: String) -> Element {
    let list_id = Uuid::parse_str(&id).ok();
    let data = use_server_future(move || {
        let lid = list_id;
        async move {
            match lid {
                Some(id) => get_basic_auth_list(id).await,
                None => Err(ServerFnError::new("invalid ID")),
            }
        }
    })?;

    let list = match &*data.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let mut new_username = use_signal(String::new);
    let mut new_password = use_signal(String::new);
    let mut adding = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut removing: Signal<Option<Uuid>> = use_signal(|| None);
    let mut deleting = use_signal(|| false);

    rsx! {
        PageHeader { "{list.name}" }

        // Info
        Card { class: "mb-4",
            div { class: "p-6 grid grid-cols-2 gap-4",
                div {
                    div { class: "text-sm text-fg-muted", "Organization" }
                    div { "{list.organization_name}" }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Credentials" }
                    div { "{list.credentials.len()}" }
                }
            }
        }

        // Credentials table
        SectionHeading { "Credentials" }
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Username" } Th { "Created" } Th { "" } } }
                    tbody {
                        if list.credentials.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "3", "No credentials" } }
                        }
                        for cred in &list.credentials {
                            {
                                let cid = cred.id;
                                let is_removing = *removing.read() == Some(cid);
                                rsx! {
                                    tr {
                                        Td { class: "font-mono", "{cred.username}" }
                                        Td { "{cred.created_at}" }
                                        Td {
                                            Button {
                                                variant: ButtonVariant::Danger,
                                                disabled: is_removing,
                                                onclick: {
                                                    let lid = list.id.to_string();
                                                    move |_| {
                                                        let lid = lid.clone();
                                                        removing.set(Some(cid));
                                                        spawn(async move {
                                                            let _ = remove_credential(cid).await;
                                                            removing.set(None);
                                                            navigator().replace(crate::web::app::Route::BasicAuthDetail { id: lid });
                                                        });
                                                    }
                                                },
                                                if is_removing { "..." } else { "Remove" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add credential form
            div { class: "p-4 border-t border-line-soft",
                div { class: "flex items-end gap-3",
                    FormField { label: "Username",
                        input {
                            class: "input w-48 font-mono",
                            r#type: "text",
                            placeholder: "username",
                            value: "{new_username}",
                            oninput: move |evt| new_username.set(evt.value()),
                        }
                    }
                    FormField { label: "Password",
                        input {
                            class: "input w-48",
                            r#type: "password",
                            placeholder: "password",
                            value: "{new_password}",
                            oninput: move |evt| new_password.set(evt.value()),
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: new_username.read().is_empty() || new_password.read().is_empty() || *adding.read(),
                        onclick: {
                            let lid = list.id;
                            let lid_str = list.id.to_string();
                            move |_| {
                                let u = new_username.read().clone();
                                let p = new_password.read().clone();
                                let lid_str = lid_str.clone();
                                adding.set(true);
                                error.set(None);
                                spawn(async move {
                                    match add_credential(lid, u, p).await {
                                        Ok(()) => {
                                            new_username.set(String::new());
                                            new_password.set(String::new());
                                            navigator().replace(crate::web::app::Route::BasicAuthDetail { id: lid_str });
                                        }
                                        Err(e) => error.set(Some(format!("{e}"))),
                                    }
                                    adding.set(false);
                                });
                            }
                        },
                        if *adding.read() { "Adding..." } else { "Add Credential" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-2 text-danger text-sm", "{err}" }
                }
            }
        }

        // Delete list
        SectionHeading { class: "mt-6", "Danger Zone" }
        Card {
            div { class: "p-4 flex items-center justify-between",
                div {
                    div { class: "text-sm font-medium", "Delete this list" }
                    div { class: "text-sm text-fg-muted", "Webspaces using this list will have their auth mode reset to none." }
                }
                Button {
                    variant: ButtonVariant::Danger,
                    disabled: *deleting.read(),
                    onclick: {
                        let lid = list.id;
                        move |_| {
                            deleting.set(true);
                            spawn(async move {
                                let _ = delete_basic_auth_list(lid).await;
                                navigator().push(crate::web::app::Route::BasicAuthList {});
                            });
                        }
                    },
                    if *deleting.read() { "Deleting..." } else { "Delete List" }
                }
            }
        }
    }
}
