use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::organizations::{
    OrgAvailableClustersInput, OrgAvailableUsersInput, OrgClusterAddInput, OrgClusterRemoveInput,
    OrgClustersInput, OrgDeleteInput, OrgGetInput, OrgMemberAddInput, OrgMemberRemoveInput,
    OrgMemberSetRoleInput, OrgMembersInput, OrgPermissions, OrgPermissionsInput,
    OrgTokenCreateInput, OrgTokenRevokeInput, OrgTokensListInput, OrgUpdateInput, add_org_cluster,
    add_org_member, change_member_role, create_org_token, delete_organization,
    get_available_clusters, get_available_users, get_org_clusters, get_org_members,
    get_org_permissions, get_organization, list_org_tokens, remove_org_cluster, remove_org_member,
    rename_organization, revoke_org_token,
};
use crate::web::app::Route;
use crate::web::components::organization_client_cas::OrganizationClientCas;
use crate::web::components::organization_client_certs::OrganizationClientCerts;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, ErrorText, HelpText,
    SectionHeading, TokenCreateForm, TokenCreateInput, TokenReveal, TokenRow, TokenTable,
};

fn role_variant(role: &str) -> BadgeVariant {
    match role {
        "admin" => BadgeVariant::Accent,
        "write" => BadgeVariant::Info,
        _ => BadgeVariant::Neutral,
    }
}

#[component]
pub fn OrganizationDetail(id: String) -> Element {
    use_topbar(t!("nav-organizations"), None);
    let id_for_org = id.clone();
    let mut org_future = use_server_future(move || {
        let id = id_for_org.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_organization(OrgGetInput { id }).await
        }
    })?;

    let id_for_perms = id.clone();
    let perms_future = use_server_future(move || {
        let id = id_for_perms.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_org_permissions(OrgPermissionsInput { id }).await
        }
    })?;

    let id_for_members = id.clone();
    let mut members_future = use_server_future(move || {
        let id = id_for_members.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_org_members(OrgMembersInput { id }).await
        }
    })?;

    let id_for_clusters = id.clone();
    let mut clusters_future = use_server_future(move || {
        let id = id_for_clusters.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_org_clusters(OrgClustersInput { id }).await
        }
    })?;

    let id_for_avail_users = id.clone();
    let mut avail_users_future = use_server_future(move || {
        let id = id_for_avail_users.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_available_users(OrgAvailableUsersInput { id }).await
        }
    })?;

    let id_for_avail_clusters = id.clone();
    let mut avail_clusters_future = use_server_future(move || {
        let id = id_for_avail_clusters.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_available_clusters(OrgAvailableClustersInput { id }).await
        }
    })?;

    let id_for_tokens = id.clone();
    let mut tokens_future = use_server_future(move || {
        let id = id_for_tokens.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_org_tokens(OrgTokensListInput { id }).await
        }
    })?;

    let mut selected_user = use_signal(|| Option::<String>::None);
    let mut selected_role = use_signal(|| "read".to_string());
    let mut selected_cluster = use_signal(|| Option::<String>::None);
    let mut confirm_delete = use_signal(|| false);
    let mut created_token = use_signal(|| Option::<String>::None);
    let mut editing_name = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let nav = navigator();

    let perms = match &*perms_future.read() {
        Some(Ok(p)) => p.clone(),
        _ => OrgPermissions {
            is_global_admin: false,
            is_org_admin: false,
        },
    };

    match &*org_future.read() {
        Some(Ok(info)) => {
            let members = match &*members_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let clusters = match &*clusters_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let avail_users = match &*avail_users_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let avail_clusters = match &*avail_clusters_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };
            let tokens = match &*tokens_future.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            let can_manage_members = perms.is_org_admin;
            let can_manage_clusters = perms.is_global_admin;
            let can_manage_tokens = perms.is_org_admin;
            let can_delete = perms.is_global_admin;

            let org_name = info.name.clone();
            let oid_for_rename = id.clone();
            let created = info.created_at.format("%Y-%m-%d %H:%M").to_string();
            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        div { class: "flex items-center gap-3",
                            if *editing_name.read() {
                                form { class: "flex items-center gap-2",
                                    onsubmit: move |evt: FormEvent| {
                                        evt.prevent_default();
                                        let oid = oid_for_rename.clone();
                                        let new_name = draft_name.read().clone();
                                        async move {
                                            if !new_name.trim().is_empty() {
                                                if let Ok(id) = oid.parse::<uuid::Uuid>() {
                                                    let _ = rename_organization(OrgUpdateInput {
                                                        id,
                                                        name: new_name,
                                                    })
                                                    .await;
                                                    org_future.restart();
                                                }
                                            }
                                            editing_name.set(false);
                                        }
                                    },
                                    input {
                                        class: "input text-2xl font-bold w-auto py-1",
                                        r#type: "text",
                                        value: "{draft_name}",
                                        oninput: move |e| draft_name.set(e.value()),
                                        autofocus: true,
                                    }
                                    button { class: "text-success hover:opacity-80", r#type: "submit",
                                        {t!("save")}
                                    }
                                    button { class: "text-fg-muted hover:text-fg-strong", r#type: "button",
                                        onclick: move |_| editing_name.set(false),
                                        {t!("cancel")}
                                    }
                                }
                            } else {
                                h1 { class: "h-page mb-0", "{info.name}" }
                                if perms.is_org_admin {
                                    button { class: "text-fg-faint hover:text-fg-muted",
                                        onclick: move |_| {
                                            draft_name.set(org_name.clone());
                                            editing_name.set(true);
                                        },
                                        {t!("edit")}
                                    }
                                }
                            }
                        }
                        p { class: "help",
                            {t!("org-detail-created", date: created)}
                        }
                    }
                    if can_delete {
                        div { class: "flex gap-2",
                            if *confirm_delete.read() {
                                span { class: "text-sm text-danger self-center mr-2", {t!("org-detail-confirm")} }
                                Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                    onclick: {
                                        let oid = id.clone();
                                        move |_| {
                                            let oid = oid.clone();
                                            async move {
                                                if let Ok(id) = oid.parse::<uuid::Uuid>() {
                                                    let _ = delete_organization(OrgDeleteInput { id }).await;
                                                }
                                                nav.push(Route::OrganizationList {});
                                            }
                                        }
                                    },
                                    {t!("org-detail-confirm-delete")}
                                }
                                Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm,
                                    onclick: move |_| confirm_delete.set(false),
                                    {t!("cancel")}
                                }
                            } else {
                                Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                                    onclick: move |_| confirm_delete.set(true),
                                    {t!("org-detail-delete")}
                                }
                            }
                        }
                    }
                }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    // Members section
                    Card { class: "p-4",
                        SectionHeading { {t!("org-detail-members")} }

                        if can_manage_members {
                            div { class: "flex gap-2 mb-4",
                                select { class: "input flex-1 w-auto py-1 text-sm",
                                    onchange: move |e| {
                                        let val = e.value();
                                        if val.is_empty() {
                                            selected_user.set(None);
                                        } else {
                                            selected_user.set(Some(val));
                                        }
                                    },
                                    option { value: "", {t!("org-detail-select-user")} }
                                    for u in &avail_users {
                                        {
                                            let uid = u.id.clone();
                                            let label = format!("{} ({})", u.email, u.name);
                                            rsx! { option { value: "{uid}", "{label}" } }
                                        }
                                    }
                                }
                                select { class: "input w-24 py-1 text-sm",
                                    value: "{selected_role}",
                                    onchange: move |e| selected_role.set(e.value()),
                                    option { value: "read", {t!("org-detail-role-read")} }
                                    option { value: "write", {t!("org-detail-role-write")} }
                                    option { value: "admin", {t!("org-detail-role-admin")} }
                                }
                                Button { size: ButtonSize::Sm,
                                    disabled: selected_user.read().is_none(),
                                    onclick: {
                                        let oid = id.clone();
                                        move |_| {
                                            let oid = oid.clone();
                                            let uid = selected_user.read().clone();
                                            let role = selected_role.read().clone();
                                            async move {
                                                if let Some(uid) = uid {
                                                    if let (Ok(id), Ok(user_id)) =
                                                        (oid.parse::<uuid::Uuid>(), uid.parse::<uuid::Uuid>())
                                                    {
                                                        let _ = add_org_member(OrgMemberAddInput {
                                                            id,
                                                            user_id,
                                                            role,
                                                        })
                                                        .await;
                                                        selected_user.set(None);
                                                        members_future.restart();
                                                        avail_users_future.restart();
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    {t!("add")}
                                }
                            }
                        }

                        if members.is_empty() {
                            HelpText { {t!("org-detail-no-members")} }
                        } else {
                            div { class: "divide-y divide-line-soft",
                                for m in &members {
                                    {
                                        let uid = m.user_id.clone();
                                        let oid = id.clone();
                                        let current_role = m.role.clone();
                                        let variant = role_variant(&m.role);
                                        rsx! {
                                            div { class: "flex justify-between items-center py-2",
                                                div { class: "flex items-center gap-2",
                                                    span { class: "text-sm font-medium", "{m.email}" }
                                                    if !m.name.is_empty() {
                                                        span { class: "text-sm text-fg-muted", "({m.name})" }
                                                    }
                                                    if can_manage_members {
                                                        select { class: "input input-xs w-auto",
                                                            value: "{current_role}",
                                                            onchange: {
                                                                let uid = uid.clone();
                                                                let oid = oid.clone();
                                                                move |e: Event<FormData>| {
                                                                    let uid = uid.clone();
                                                                    let oid = oid.clone();
                                                                    let new_role = e.value();
                                                                    async move {
                                                                        if let (Ok(id), Ok(user_id)) =
                                                                            (oid.parse::<uuid::Uuid>(), uid.parse::<uuid::Uuid>())
                                                                        {
                                                                            let _ = change_member_role(OrgMemberSetRoleInput {
                                                                                id,
                                                                                user_id,
                                                                                role: new_role,
                                                                            })
                                                                            .await;
                                                                            members_future.restart();
                                                                        }
                                                                    }
                                                                }
                                                            },
                                                            option { value: "read", {t!("org-detail-role-read")} }
                                                            option { value: "write", {t!("org-detail-role-write")} }
                                                            option { value: "admin", {t!("org-detail-role-admin")} }
                                                        }
                                                    } else {
                                                        Badge { variant, "{current_role}" }
                                                    }
                                                }
                                                if can_manage_members {
                                                    button { class: "link-danger text-sm",
                                                        onclick: {
                                                            let uid = uid.clone();
                                                            let oid = oid.clone();
                                                            move |_| {
                                                                let uid = uid.clone();
                                                                let oid = oid.clone();
                                                                async move {
                                                                    if let (Ok(id), Ok(user_id)) =
                                                                        (oid.parse::<uuid::Uuid>(), uid.parse::<uuid::Uuid>())
                                                                    {
                                                                        let _ = remove_org_member(OrgMemberRemoveInput {
                                                                            id,
                                                                            user_id,
                                                                        })
                                                                        .await;
                                                                        members_future.restart();
                                                                        avail_users_future.restart();
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

                    // Clusters section
                    Card { class: "p-4",
                        SectionHeading { {t!("org-detail-clusters")} }

                        if can_manage_clusters {
                            div { class: "flex gap-2 mb-4",
                                select { class: "input flex-1 w-auto py-1 text-sm",
                                    onchange: move |e| {
                                        let val = e.value();
                                        if val.is_empty() {
                                            selected_cluster.set(None);
                                        } else {
                                            selected_cluster.set(Some(val));
                                        }
                                    },
                                    option { value: "", {t!("org-detail-select-cluster")} }
                                    for c in &avail_clusters {
                                        {
                                            let cid = c.id.clone();
                                            let cname = c.name.clone();
                                            rsx! { option { value: "{cid}", "{cname}" } }
                                        }
                                    }
                                }
                                Button { size: ButtonSize::Sm,
                                    disabled: selected_cluster.read().is_none(),
                                    onclick: {
                                        let oid = id.clone();
                                        move |_| {
                                            let oid = oid.clone();
                                            let cid = selected_cluster.read().clone();
                                            async move {
                                                if let Some(cid) = cid {
                                                    if let (Ok(id), Ok(cluster_id)) =
                                                        (oid.parse::<uuid::Uuid>(), cid.parse::<uuid::Uuid>())
                                                    {
                                                        let _ = add_org_cluster(OrgClusterAddInput {
                                                            id,
                                                            cluster_id,
                                                        })
                                                        .await;
                                                        selected_cluster.set(None);
                                                        clusters_future.restart();
                                                        avail_clusters_future.restart();
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    {t!("add")}
                                }
                            }
                        }

                        if clusters.is_empty() {
                            HelpText { {t!("org-detail-no-clusters")} }
                        } else {
                            div { class: "divide-y divide-line-soft",
                                for c in &clusters {
                                    {
                                        let cid = c.cluster_id.clone();
                                        let oid = id.clone();
                                        rsx! {
                                            div { class: "flex justify-between items-center py-2",
                                                Link { to: Route::ClusterDetail { id: cid.clone() },
                                                    class: "link text-sm font-medium",
                                                    "{c.name}"
                                                }
                                                if can_manage_clusters {
                                                    button { class: "link-danger text-sm",
                                                        onclick: {
                                                            let cid = cid.clone();
                                                            let oid = oid.clone();
                                                            move |_| {
                                                                let cid = cid.clone();
                                                                let oid = oid.clone();
                                                                async move {
                                                                    if let (Ok(id), Ok(cluster_id)) =
                                                                        (oid.parse::<uuid::Uuid>(), cid.parse::<uuid::Uuid>())
                                                                    {
                                                                        let _ = remove_org_cluster(OrgClusterRemoveInput {
                                                                            id,
                                                                            cluster_id,
                                                                        })
                                                                        .await;
                                                                        clusters_future.restart();
                                                                        avail_clusters_future.restart();
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

                    // Tokens section (org admins only)
                    if can_manage_tokens {
                        Card { class: "lg:col-span-2 p-4",
                            SectionHeading { {t!("org-detail-tokens")} }

                            if let Some(raw) = &*created_token.read() {
                                TokenReveal { value: raw.clone(), label: t!("org-detail-token-created") }
                            }

                            TokenCreateForm {
                                submit_label: t!("org-detail-create-token"),
                                on_submit: {
                                    let oid = id.clone();
                                    move |input: TokenCreateInput| {
                                        let oid = oid.clone();
                                        spawn(async move {
                                            if let Ok(id) = oid.parse::<uuid::Uuid>() {
                                                if let Ok(raw) = create_org_token(OrgTokenCreateInput {
                                                    id,
                                                    label: input.label,
                                                    expires_in_secs: input.expires_in_secs,
                                                })
                                                .await
                                                {
                                                    created_token.set(Some(raw));
                                                    tokens_future.restart();
                                                }
                                            }
                                        });
                                    }
                                },
                            }

                            if tokens.is_empty() {
                                HelpText { {t!("org-detail-no-tokens")} }
                            } else {
                                TokenTable {
                                    rows: tokens.iter().map(|t| TokenRow {
                                        id: t.id.clone(),
                                        label: t.label.clone(),
                                        kind: Some(t.kind.clone()),
                                        scope: None,
                                        scope_href: None,
                                        revoked: t.revoked,
                                        expired: t.expires_at.is_some_and(|e| e < chrono::Utc::now()),
                                        created: t.created_at.format("%Y-%m-%d %H:%M").to_string(),
                                        expires: t.expires_at.map(|e| e.format("%Y-%m-%d %H:%M").to_string()),
                                    }).collect::<Vec<_>>(),
                                    show_kind: true,
                                    show_expires: true,
                                    on_revoke: {
                                        let oid = id.clone();
                                        move |tid: String| {
                                            let oid = oid.clone();
                                            spawn(async move {
                                                if let (Ok(id), Ok(token_id)) =
                                                    (oid.parse::<uuid::Uuid>(), tid.parse::<uuid::Uuid>())
                                                {
                                                    let _ = revoke_org_token(OrgTokenRevokeInput {
                                                        id,
                                                        token_id,
                                                    })
                                                    .await;
                                                    created_token.set(None);
                                                    tokens_future.restart();
                                                }
                                            });
                                        }
                                    },
                                }
                            }
                        }
                    }

                    // Client Certificates section
                    Card { class: "p-4",
                        SectionHeading { {t!("org-detail-client-certs")} }
                        p { class: "text-fg-muted text-sm mb-3",
                            {t!("org-client-certs-description")}
                        }
                        OrganizationClientCerts {
                            organization_id: id.clone(),
                            read_only: !perms.is_org_admin,
                        }
                    }

                    // Client CAs section
                    Card { class: "p-4",
                        SectionHeading { {t!("org-detail-client-cas")} }
                        p { class: "text-fg-muted text-sm mb-3",
                            {t!("org-client-cas-description")}
                        }
                        OrganizationClientCas {
                            organization_id: id.clone(),
                            read_only: !perms.is_org_admin,
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
