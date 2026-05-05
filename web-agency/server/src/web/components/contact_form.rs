use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgOption {
    id: Uuid,
    name: String,
}

#[server]
async fn list_user_orgs_for_contact() -> Result<Vec<OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let orgs = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM organizations ORDER BY name")
            .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT o.id, o.name FROM organizations o \
             JOIN organization_members om ON om.organization_id = o.id \
             WHERE om.user_id = $1 AND om.role IN ('admin', 'write') ORDER BY o.name",
        )
        .bind(user.id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };
    Ok(orgs.into_iter().map(|(id, name)| OrgOption { id, name }).collect())
}

#[server]
async fn create_contact(
    org_id: Uuid,
    label: String,
    first_name: String,
    last_name: String,
    email: String,
    phone: String,
    address1: String,
    city: String,
    country: String,
) -> Result<Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO domain_contacts (organization_id, label, first_name, last_name, email, phone, address1, city, country) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(org_id).bind(&label).bind(&first_name).bind(&last_name)
    .bind(&email).bind(&phone).bind(&address1).bind(&city).bind(&country)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to create contact: {e}")))?;

    Ok(id)
}

#[component]
pub fn ContactForm() -> Element {
    let orgs = use_server_future(list_user_orgs_for_contact)?;
    let org_list = match &*orgs.read() {
        Some(Ok(o)) => o.clone(),
        _ => vec![],
    };

    let mut label = use_signal(String::new);
    let mut first_name = use_signal(String::new);
    let mut last_name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut phone = use_signal(String::new);
    let mut address1 = use_signal(String::new);
    let mut city = use_signal(String::new);
    let mut country = use_signal(|| "US".to_string());
    let mut org_id = use_signal(|| org_list.first().map(|o| o.id.to_string()).unwrap_or_default());
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Add Contact" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let vals = (
                    org_id.read().clone(), label.read().clone(),
                    first_name.read().clone(), last_name.read().clone(),
                    email.read().clone(), phone.read().clone(),
                    address1.read().clone(), city.read().clone(),
                    country.read().clone(),
                );
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let (oid_str, l, fn_, ln, em, ph, a1, ci, co) = vals;
                    match uuid::Uuid::parse_str(&oid_str) {
                        Ok(oid) => {
                            match create_contact(oid, l, fn_, ln, em, ph, a1, ci, co).await {
                                Ok(_) => { nav.push(crate::web::app::Route::ContactList {}); }
                                Err(e) => error.set(Some(format!("{e}"))),
                            }
                        }
                        Err(_) => error.set(Some("Select an organization".into())),
                    }
                    saving.set(false);
                });
            },

            FormField { label: "Organization",
                select {
                    class: "input", value: "{org_id}",
                    oninput: move |evt| org_id.set(evt.value()),
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
                    }
                }
            }
            FormField { label: "Label",
                input { class: "input", r#type: "text", placeholder: "e.g. Default Contact", required: true,
                    value: "{label}", oninput: move |evt| label.set(evt.value()) }
            }
            div { class: "grid grid-cols-2 gap-4",
                FormField { label: "First Name",
                    input { class: "input", r#type: "text", required: true,
                        value: "{first_name}", oninput: move |evt| first_name.set(evt.value()) }
                }
                FormField { label: "Last Name",
                    input { class: "input", r#type: "text", required: true,
                        value: "{last_name}", oninput: move |evt| last_name.set(evt.value()) }
                }
            }
            FormField { label: "Email",
                input { class: "input", r#type: "email", required: true,
                    value: "{email}", oninput: move |evt| email.set(evt.value()) }
            }
            FormField { label: "Phone",
                input { class: "input", r#type: "tel", required: true, placeholder: "+1234567890",
                    value: "{phone}", oninput: move |evt| phone.set(evt.value()) }
            }
            FormField { label: "Address",
                input { class: "input", r#type: "text", required: true,
                    value: "{address1}", oninput: move |evt| address1.set(evt.value()) }
            }
            div { class: "grid grid-cols-2 gap-4",
                FormField { label: "City",
                    input { class: "input", r#type: "text", required: true,
                        value: "{city}", oninput: move |evt| city.set(evt.value()) }
                }
                FormField { label: "Country (ISO 2-letter)",
                    input { class: "input", r#type: "text", required: true, maxlength: "2",
                        value: "{country}", oninput: move |evt| country.set(evt.value()) }
                }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-red-400 text-sm", "{err}" }
            }

            div { class: "flex gap-3",
                Button {
                    variant: ButtonVariant::Primary, kind: ButtonKind::Submit, disabled: *saving.read(),
                    if *saving.read() { "Saving..." } else { "Create Contact" }
                }
                Link { to: crate::web::app::Route::ContactList {}, class: "btn btn-secondary", "Cancel" }
            }
        }
    }
}
