use dioxus::prelude::*;
use uuid::Uuid;

use super::ui::{Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, Th};

// The basic-auth list read/mutation endpoints now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::basic_auth::{
    BasicAuthAddCredentialInput, BasicAuthDeleteInput, BasicAuthGetInput,
    BasicAuthRemoveCredentialInput, add_basic_auth_credential, delete_basic_auth_list,
    get_basic_auth_list, remove_basic_auth_credential,
};

#[component]
pub fn BasicAuthDetail(id: String) -> Element {
    let list_id = Uuid::parse_str(&id).ok();
    let data = use_server_future(move || {
        let lid = list_id;
        async move {
            match lid {
                Some(id) => get_basic_auth_list(BasicAuthGetInput { id }).await,
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
                                                    let list_id = list.id;
                                                    let lid = list.id.to_string();
                                                    move |_| {
                                                        let lid = lid.clone();
                                                        removing.set(Some(cid));
                                                        spawn(async move {
                                                            let _ = remove_basic_auth_credential(BasicAuthRemoveCredentialInput {
                                                                id: list_id,
                                                                credential_id: cid,
                                                            }).await;
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
                                    match add_basic_auth_credential(BasicAuthAddCredentialInput {
                                        id: lid,
                                        username: u,
                                        password: p,
                                    }).await {
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
                                let _ = delete_basic_auth_list(BasicAuthDeleteInput { id: lid }).await;
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
