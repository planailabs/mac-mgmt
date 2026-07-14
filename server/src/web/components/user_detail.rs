use dioxus::prelude::*;

use dioxus_i18n::t;

use crate::api_mcp::endpoints::users::{
    UserAvailableOrgsInput, UserDeleteInput, UserGetInput, UserOrgAddInput, UserOrgRemoveInput,
    UserOrgsInput, UserSetAdminInput, add_user_to_org, delete_user, get_available_orgs_for_user,
    get_user, get_user_orgs, remove_user_from_org, toggle_user_admin,
};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, ErrorText, HelpText, Kicker,
    SectionHeading,
};

fn role_variant(role: &str) -> BadgeVariant {
    match role {
        "admin" => BadgeVariant::Accent,
        "write" => BadgeVariant::Info,
        _ => BadgeVariant::Neutral,
    }
}

#[component]
pub fn UserDetail(id: String) -> Element {
    use_topbar(t!("nav-users"), None);
    let id_for_user = id.clone();
    let mut user_future = use_server_future(move || {
        let id = id_for_user.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_user(UserGetInput { id }).await
        }
    })?;

    let id_for_orgs = id.clone();
    let mut orgs_future = use_server_future(move || {
        let id = id_for_orgs.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_user_orgs(UserOrgsInput { id }).await
        }
    })?;

    let id_for_avail = id.clone();
    let mut avail_future = use_server_future(move || {
        let id = id_for_avail.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_available_orgs_for_user(UserAvailableOrgsInput { id }).await
        }
    })?;

    let mut selected_org = use_signal(|| Option::<String>::None);
    let mut selected_org_role = use_signal(|| "read".to_string());
    let mut confirm_delete = use_signal(|| false);
    let nav = navigator();

    match &*user_future.read() {
        Some(Ok(info)) => {
            let orgs = match &*orgs_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let avail_orgs = match &*avail_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            let is_admin = info.is_admin;
            let user_id = info.id.clone();
            let created = info.created_at.format("%Y-%m-%d %H:%M").to_string();

            rsx! {
                div { class: "flex flex-col sm:flex-row sm:justify-between sm:items-end gap-3 mb-4",
                    div { class: "min-w-0",
                        Kicker { class: "mb-2", {t!("nav-users")} }
                        h1 { class: "h-page mb-0 break-all", "{info.email}" }
                        p { class: "help mt-2",
                            if !info.name.is_empty() {
                                span { "{info.name} · " }
                            }
                            {t!("user-detail-created", date: created)}
                        }
                    }
                    div { class: "flex gap-2 items-center",
                        // Admin toggle
                        div { class: "flex items-center gap-2 mr-4",
                            input {
                                r#type: "checkbox",
                                checked: is_admin,
                                class: "h-4 w-4 rounded border-line text-brand focus:ring-brand",
                                onchange: {
                                    let uid = user_id.clone();
                                    move |e: Event<FormData>| {
                                        let uid = uid.clone();
                                        let new_val = e.checked();
                                        async move {
                                            if let Ok(id) = uid.parse::<uuid::Uuid>() {
                                                let _ = toggle_user_admin(UserSetAdminInput {
                                                    id,
                                                    is_admin: new_val,
                                                })
                                                .await;
                                                user_future.restart();
                                            }
                                        }
                                    }
                                },
                            }
                            label { class: "text-sm font-medium text-fg-strong", {t!("admin")} }
                        }
                        // Impersonate
                        Button { variant: ButtonVariant::Warn, size: ButtonSize::Sm,
                            onclick: {
                                let uid = user_id.clone();
                                move |_| {
                                    // The cookie is HttpOnly+Secure+SameSite=Strict and can
                                    // only be set by the server-side endpoint. uid here is
                                    // bound from the page's :id route param (a UUID), so it
                                    // is safe to interpolate into the URL path.
                                    let js = format!(
                                        "fetch('/auth/impersonate/start/{uid}', {{method:'POST',credentials:'same-origin'}}).then(r=>{{ if(r.ok){{ window.location.href='/'; }} }});"
                                    );
                                    document::eval(&js);
                                }
                            },
                            {t!("user-detail-impersonate")}
                        }
                        // Delete (with confirm flow)
                        if *confirm_delete.read() {
                            span { class: "text-sm text-danger mr-2", {t!("user-detail-confirm")} }
                            Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                onclick: {
                                    let uid = id.clone();
                                    move |_| {
                                        let uid = uid.clone();
                                        async move {
                                            if let Ok(id) = uid.parse::<uuid::Uuid>() {
                                                let _ = delete_user(UserDeleteInput { id }).await;
                                            }
                                            nav.push(Route::UserList {});
                                        }
                                    }
                                },
                                {t!("user-detail-yes-delete")}
                            }
                            Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm,
                                onclick: move |_| confirm_delete.set(false),
                                {t!("cancel")}
                            }
                        } else {
                            Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                onclick: move |_| confirm_delete.set(true),
                                {t!("user-detail-delete")}
                            }
                        }
                    }
                }

                // Organizations section
                Card { class: "p-4",
                    SectionHeading { {t!("user-detail-orgs")} }

                    div { class: "flex gap-2 mb-4",
                        select {
                            class: "input flex-1 w-auto py-1 text-sm",
                            onchange: move |e| {
                                let val = e.value();
                                if val.is_empty() {
                                    selected_org.set(None);
                                } else {
                                    selected_org.set(Some(val));
                                }
                            },
                            option { value: "", {t!("user-detail-select-org")} }
                            for o in &avail_orgs {
                                {
                                    let oid = o.id.clone();
                                    let oname = o.name.clone();
                                    rsx! { option { value: "{oid}", "{oname}" } }
                                }
                            }
                        }
                        select {
                            class: "input w-24 py-1 text-sm",
                            value: "{selected_org_role}",
                            onchange: move |e| selected_org_role.set(e.value()),
                            option { value: "read", {t!("org-detail-role-read")} }
                            option { value: "write", {t!("org-detail-role-write")} }
                            option { value: "admin", {t!("org-detail-role-admin")} }
                        }
                        Button { size: ButtonSize::Sm,
                            disabled: selected_org.read().is_none(),
                            onclick: {
                                let uid = id.clone();
                                move |_| {
                                    let uid = uid.clone();
                                    let oid = selected_org.read().clone();
                                    let role = selected_org_role.read().clone();
                                    async move {
                                        if let Some(oid) = oid {
                                            if let (Ok(id), Ok(org_id)) =
                                                (uid.parse::<uuid::Uuid>(), oid.parse::<uuid::Uuid>())
                                            {
                                                let _ = add_user_to_org(UserOrgAddInput {
                                                    id,
                                                    org_id,
                                                    role,
                                                })
                                                .await;
                                                selected_org.set(None);
                                                orgs_future.restart();
                                                avail_future.restart();
                                            }
                                        }
                                    }
                                }
                            },
                            {t!("add")}
                        }
                    }

                    if orgs.is_empty() {
                        HelpText { {t!("user-detail-no-orgs")} }
                    } else {
                        div { class: "divide-y divide-line-soft",
                            for o in &orgs {
                                {
                                    let oid = o.organization_id.clone();
                                    let uid = id.clone();
                                    let oname = o.name.clone();
                                    let role = o.role.clone();
                                    let variant = role_variant(&o.role);
                                    rsx! {
                                        div { class: "flex justify-between items-center py-2",
                                            div { class: "flex items-center gap-2",
                                                Link { to: Route::OrganizationDetail { id: oid.clone() },
                                                    class: "link text-sm font-medium",
                                                    "{oname}"
                                                }
                                                Badge { variant, "{role}" }
                                            }
                                            button { class: "link-danger text-sm",
                                                onclick: {
                                                    let oid = oid.clone();
                                                    let uid = uid.clone();
                                                    move |_| {
                                                        let oid = oid.clone();
                                                        let uid = uid.clone();
                                                        async move {
                                                            if let (Ok(id), Ok(org_id)) =
                                                                (uid.parse::<uuid::Uuid>(), oid.parse::<uuid::Uuid>())
                                                            {
                                                                let _ = remove_user_from_org(UserOrgRemoveInput {
                                                                    id,
                                                                    org_id,
                                                                })
                                                                .await;
                                                                orgs_future.restart();
                                                                avail_future.restart();
                                                            }
                                                        }
                                                    }
                                                },
                                                {t!("remove")}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
