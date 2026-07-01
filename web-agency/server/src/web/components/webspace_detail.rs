use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td,
    TdMuted, Th, TokenCreateForm, TokenCreateInput, TokenReveal, TokenRow, TokenTable,
};

// Most webspace server fns now live in the shared api_mcp layer; only the
// deploy-token fns and pure dropdown loaders remain here.
use crate::api_mcp::endpoints::webspaces::{
    BuildConfigInfo, GitRepoInfo, WebspaceConnectGitInput, WebspaceDeleteInput,
    WebspaceDeployPagesInput, WebspaceDeploymentsInput, WebspaceGetInput, WebspaceMoveInput,
    WebspaceSetProductionBranchInput, WebspaceUpdateInput, connect_git_repo, delete_webspace,
    deploy_pages_project, get_webspace, list_deployments, move_webspace,
    update_production_branch, update_webspace_settings,
};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCreateResult {
    token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn list_cf_creds_for_pages() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| CredOption { id, name })
        .collect())
}

// ── Deployments & tokens ─────────────────────────────────────────────

#[server]
async fn create_deploy_token(
    webspace_id: Uuid,
    org_id: Uuid,
    label: String,
    expires_in_secs: Option<i64>,
) -> Result<TokenCreateResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    // Require org admin (not just write) for token creation.
    use crate::web::user::WebUserExt;
    user.require_org_admin(&org_id)?;

    // Verify the webspace belongs to this org.
    let ws_org =
        sqlx::query_scalar::<_, Uuid>("SELECT organization_id FROM webspaces WHERE id = $1")
            .bind(webspace_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    if ws_org != org_id {
        return Err(ServerFnError::new(
            "webspace does not belong to this organization",
        ));
    }

    use rand::Rng;
    let token_bytes: [u8; 32] = rand::rng().random();
    let token = hex::encode(token_bytes);

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let scopes = serde_json::json!({ "webspace_id": webspace_id.to_string() });
    let expires_at = expires_in_secs.map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label, kind, scopes, expires_at) VALUES ($1, $2, $3, 'deploy', $4, $5)",
    )
    .bind(org_id)
    .bind(&hash)
    .bind(&label)
    .bind(&scopes)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(TokenCreateResult { token })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebspaceTokenInfo {
    id: Uuid,
    label: String,
    revoked: bool,
    created_at: String,
    expires_at: Option<String>,
    expired: bool,
}

/// Deploy tokens scoped to this webspace (org admin only).
#[server]
async fn list_webspace_tokens(
    webspace_id: Uuid,
    org_id: Uuid,
) -> Result<Vec<WebspaceTokenInfo>, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_org_admin(&org_id)?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, (Uuid, String, bool, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, label, revoked, created_at, expires_at FROM tokens \
         WHERE kind = 'deploy' AND organization_id = $1 \
           AND (scopes->>'webspace_id')::uuid = $2 \
         ORDER BY created_at DESC",
    )
    .bind(org_id)
    .bind(webspace_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let now = chrono::Utc::now();
    Ok(rows
        .into_iter()
        .map(|(id, label, revoked, created_at, expires_at)| WebspaceTokenInfo {
            id,
            label,
            revoked,
            created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            expires_at: expires_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
            expired: expires_at.is_some_and(|e| e < now),
        })
        .collect())
}

#[server]
async fn revoke_webspace_token(token_id: Uuid, org_id: Uuid) -> Result<(), ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_org_admin(&org_id)?;
    let pool = crate::server_pool()?;
    // Scope the update to the org so an admin can't revoke another org's token
    // by guessing an id.
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1 AND organization_id = $2")
        .bind(token_id)
        .bind(org_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn WebspaceDetail(id: String) -> Element {
    let webspace_id = Uuid::parse_str(&id).ok();
    let refresh = use_context_provider(|| Signal::new(0u32));
    let webspace = use_server_future(move || {
        let wid = webspace_id;
        let _ = *refresh.read(); // reactive dependency — bumping refresh re-runs this future
        async move {
            match wid {
                Some(id) => get_webspace(WebspaceGetInput { id }).await,
                None => Err(ServerFnError::new("invalid webspace ID")),
            }
        }
    })?;

    let data = match &*webspace.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let is_pages = data.hosting_type == "cloudflare_pages";
    let has_project = data.cloudflare_pages_project.is_some();
    // Local static folders accept the same tarball/token deploy flow as Pages.
    let is_static = data.hosting_type == "local" && data.runtime.as_deref() == Some("static");

    rsx! {
        PageHeader { "{data.name}" }

        // Info card
        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div {
                    div { class: "text-sm text-fg-muted", "Hosting Type" }
                    match data.hosting_type.as_str() {
                        "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "Cloudflare Pages" } },
                        "local" => rsx! { Badge { "Local" } },
                        "relay" => rsx! { Badge { variant: BadgeVariant::Accent, "Relay Tunnel" } },
                        "tunnel" => rsx! { Badge { variant: BadgeVariant::Accent, "Tunnel" } },
                        _ => rsx! { span { "{data.hosting_type}" } },
                    }
                }
                if is_pages {
                    div {
                        div { class: "text-sm text-fg-muted", "Pages Project" }
                        if let Some(ref proj) = data.cloudflare_pages_project {
                            span { class: "font-mono text-sm", "{proj}" }
                        } else {
                            span { class: "text-fg-muted", "Not deployed" }
                        }
                    }
                } else if data.hosting_type == "relay" || data.hosting_type == "tunnel" {
                    div {
                        div { class: "text-sm text-fg-muted", "Upstream URL" }
                        if let Some(ref url) = data.relay_url {
                            span { class: "font-mono text-sm break-all", "{url}" }
                        } else {
                            span { class: "text-fg-muted", "-" }
                        }
                    }
                } else {
                    div {
                        div { class: "text-sm text-fg-muted", "Runtime" }
                        div { {data.runtime.as_deref().unwrap_or("-")} }
                    }
                    div {
                        div { class: "text-sm text-fg-muted", "Status" }
                        match data.local_status.as_deref() {
                            Some("running") => rsx! { Badge { variant: BadgeVariant::Success, "Running" } },
                            Some("error") => rsx! { Badge { variant: BadgeVariant::Danger, "Error" } },
                            Some(s) => rsx! { Badge { "{s}" } },
                            None => rsx! { span { class: "text-fg-muted", "-" } },
                        }
                    }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Organization" }
                    div { "{data.organization_name}" }
                }
                if !is_pages {
                    div {
                        div { class: "text-sm text-fg-muted", "Auth" }
                        match data.auth_mode.as_str() {
                            "oidc" => rsx! { Badge { variant: BadgeVariant::Info, "OIDC" } },
                            "basic" => rsx! {
                                div { class: "flex items-center gap-1",
                                    Badge { variant: BadgeVariant::Accent, "Basic" }
                                    if let Some(ref list_name) = data.auth_basic_list_name {
                                        span { class: "text-sm text-fg-muted", "({list_name})" }
                                    }
                                }
                            },
                            _ => rsx! { span { class: "text-fg-muted", "None" } },
                        }
                    }
                }
            }
        }

        // Settings (editable name + upstream URL)
        SectionHeading { class: "mt-6", "Settings" }
        WebspaceSettingsSection {
            webspace_id: data.id,
            current_name: data.name.clone(),
            hosting_type: data.hosting_type.clone(),
            current_relay_url: data.relay_url.clone(),
        }

        // CF Pages deployment
        if is_pages && !has_project {
            SectionHeading { class: "mt-6", "Deploy Pages Project" }
            PagesDeploySection { webspace_id: data.id }
        }

        // Source info (only for deployed CF Pages projects)
        if is_pages && has_project {
            SectionHeading { class: "mt-6", "Deployment Source" }
            if data.git_source.is_some() {
                // Git-connected project
                GitSourceDisplay {
                    git_source: data.git_source.clone().unwrap(),
                    build_config: data.build_config.clone(),
                    pages_subdomain: data.pages_subdomain.clone(),
                }
            } else {
                // Direct upload project
                DirectUploadDisplay {
                    webspace_id: data.id,
                    project_name: data.cloudflare_pages_project.clone().unwrap_or_default(),
                    production_branch: data.production_branch.clone().unwrap_or_else(|| "main".into()),
                    pages_subdomain: data.pages_subdomain.clone(),
                }
            }
        }

        // Deployments (direct-upload Pages projects and local static folders)
        if (is_pages && has_project && data.git_source.is_none()) || is_static {
            SectionHeading { class: "mt-6", "Deployments" }
            DeploymentsSection { webspace_id: data.id }
        }

        // Deploy token (org admins can create inline)
        if ((is_pages && has_project && data.git_source.is_none()) || is_static) && data.is_org_admin {
            SectionHeading { class: "mt-6", "Deploy Token" }
            DeployTokenSection { webspace_id: data.id, organization_id: data.organization_id }
        }

        // Auth settings (not for Pages)
        if !is_pages {
            SectionHeading { class: "mt-6", "Auth" }
            AuthSettingsSection {
                webspace_id: data.id,
                organization_id: data.organization_id,
                current_mode: data.auth_mode.clone(),
                current_list_name: data.auth_basic_list_name.clone(),
            }
        }

        // Domains, CNAME and Change Detection are managed on the parent host.
        SectionHeading { class: "mt-6", "Host" }
        Card {
            div { class: "p-6 flex items-center gap-3",
                span { class: "text-sm text-fg-muted", "This folder is mounted at" }
                Badge { "{data.path_prefix}" }
                span { class: "text-sm text-fg-muted", "on host" }
                Link {
                    to: crate::web::app::Route::WebspaceHostDetail { id: data.webspace_host_id.to_string() },
                    class: "text-brand underline",
                    "{data.host_name}"
                }
                span { class: "text-sm text-fg-muted", "— manage domains, CNAME and change detection there." }
            }
        }

        // Danger zone
        SectionHeading { class: "mt-6", "Danger Zone" }
        MoveWebspaceSection { webspace_id: data.id, current_org_id: data.organization_id }
        DeleteWebspaceSection { webspace_id: data.id }
    }
}

/// Git repo connection / display section.
#[component]
fn GitRepoSection(
    webspace_id: Uuid,
    git_source: Option<GitRepoInfo>,
    build_config: Option<BuildConfigInfo>,
    pages_subdomain: Option<String>,
) -> Element {
    if let Some(ref git) = git_source {
        // Already connected — show info
        let repo_url = match git.provider.as_str() {
            "github" => format!("https://github.com/{}/{}", git.owner, git.repo),
            "gitlab" => format!("https://gitlab.com/{}/{}", git.owner, git.repo),
            _ => format!("{}/{}", git.owner, git.repo),
        };
        return rsx! {
            Card {
                div { class: "p-6 space-y-3",
                    div { class: "flex items-center gap-2",
                        Badge { variant: BadgeVariant::Success, "Connected" }
                        span { class: "font-mono text-sm", "{git.provider}" }
                    }
                    div { class: "flex items-center gap-2",
                        span { class: "text-sm text-fg-muted", "Repository:" }
                        a {
                            href: "{repo_url}",
                            target: "_blank",
                            class: "font-mono text-sm text-brand underline",
                            "{git.owner}/{git.repo}"
                        }
                    }
                    div { class: "flex items-center gap-2",
                        span { class: "text-sm text-fg-muted", "Branch:" }
                        span { class: "font-mono text-sm", "{git.production_branch}" }
                    }
                    if let Some(ref bc) = build_config {
                        if bc.build_command.is_some() || bc.destination_dir.is_some() {
                            div { class: "border-t border-line-soft pt-3 mt-3",
                                div { class: "text-sm text-fg-muted mb-1", "Build Settings" }
                                if let Some(ref cmd) = bc.build_command {
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm text-fg-muted", "Command:" }
                                        span { class: "font-mono text-sm bg-surface-2 px-2 py-0.5 rounded", "{cmd}" }
                                    }
                                }
                                if let Some(ref dir) = bc.destination_dir {
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm text-fg-muted", "Output:" }
                                        span { class: "font-mono text-sm", "{dir}" }
                                    }
                                }
                                if let Some(ref root) = bc.root_dir {
                                    div { class: "flex items-center gap-2",
                                        span { class: "text-sm text-fg-muted", "Root:" }
                                        span { class: "font-mono text-sm", "{root}" }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(ref sub) = pages_subdomain {
                        div { class: "flex items-center gap-2",
                            span { class: "text-sm text-fg-muted", "Preview:" }
                            a {
                                href: "https://{sub}",
                                target: "_blank",
                                class: "font-mono text-sm text-brand underline",
                                "https://{sub}"
                            }
                        }
                    }
                }
            }
        };
    }

    // Not connected — show form
    let mut refresh: Signal<u32> = use_context();
    let mut provider = use_signal(|| "github".to_string());
    let mut owner = use_signal(String::new);
    let mut repo_name = use_signal(String::new);
    let mut branch = use_signal(|| "main".to_string());
    let mut build_cmd = use_signal(String::new);
    let mut dest_dir = use_signal(String::new);
    let mut root_dir = use_signal(String::new);
    let mut connecting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                p { class: "text-fg-muted text-sm mb-2",
                    "Connect a GitHub or GitLab repository. The Cloudflare GitHub/GitLab integration must be "
                    a { href: "https://dash.cloudflare.com/?to=/:account/pages", target: "_blank", class: "text-brand underline", "authorized in your Cloudflare dashboard" }
                    " first."
                }

                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Provider",
                        select {
                            class: "input",
                            value: "{provider}",
                            oninput: move |evt| provider.set(evt.value()),
                            option { value: "github", "GitHub" }
                            option { value: "gitlab", "GitLab" }
                        }
                    }
                    FormField { label: "Production Branch",
                        input { class: "input", r#type: "text", value: "{branch}",
                            placeholder: "main",
                            oninput: move |evt| branch.set(evt.value()) }
                    }
                }

                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Owner (user or org)",
                        input { class: "input", r#type: "text", required: true,
                            placeholder: "my-github-org",
                            value: "{owner}", oninput: move |evt| owner.set(evt.value()) }
                    }
                    FormField { label: "Repository Name",
                        input { class: "input", r#type: "text", required: true,
                            placeholder: "my-website",
                            value: "{repo_name}", oninput: move |evt| repo_name.set(evt.value()) }
                    }
                }

                SectionHeading { "Build Settings (optional)" }
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                    FormField { label: "Build Command",
                        input { class: "input font-mono text-sm", r#type: "text",
                            placeholder: "npm run build",
                            value: "{build_cmd}", oninput: move |evt| build_cmd.set(evt.value()) }
                    }
                    FormField { label: "Output Directory",
                        input { class: "input font-mono text-sm", r#type: "text",
                            placeholder: "dist",
                            value: "{dest_dir}", oninput: move |evt| dest_dir.set(evt.value()) }
                    }
                    FormField { label: "Root Directory",
                        help: "Subdirectory where the project lives (leave empty for repo root)",
                        input { class: "input font-mono text-sm", r#type: "text",
                            placeholder: "/",
                            value: "{root_dir}", oninput: move |evt| root_dir.set(evt.value()) }
                    }
                }

                if let Some(err) = &*error.read() {
                    div { class: "text-danger text-sm", "{err}" }
                }

                Button {
                    variant: ButtonVariant::Primary,
                    disabled: owner.read().is_empty() || repo_name.read().is_empty() || *connecting.read(),
                    onclick: {
                        let wid = webspace_id;
                        move |_| {
                            let p = provider.read().clone();
                            let o = owner.read().clone();
                            let r = repo_name.read().clone();
                            let b = branch.read().clone();
                            let bc = build_cmd.read().clone();
                            let dd = dest_dir.read().clone();
                            let rd = root_dir.read().clone();
                            connecting.set(true);
                            error.set(None);
                            spawn(async move {
                                match connect_git_repo(WebspaceConnectGitInput {
                                    id: wid,
                                    provider: p,
                                    owner: o,
                                    repo_name: r,
                                    production_branch: b,
                                    build_command: bc,
                                    destination_dir: dd,
                                    root_dir: rd,
                                }).await {
                                    Ok(()) => {
                                        refresh += 1;
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("{e}")));
                                        connecting.set(false);
                                    }
                                }
                            });
                        }
                    },
                    if *connecting.read() { "Connecting..." } else { "Connect Repository" }
                }
            }
        }
    }
}

/// Deploy a CF Pages project — user chooses Git or Direct Upload (cannot be changed later).
#[component]
fn PagesDeploySection(webspace_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let creds = use_server_future(list_cf_creds_for_pages)?;
    let cred_list = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut cred_id = use_signal(|| {
        cred_list
            .first()
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    });
    let mut mode = use_signal(|| "direct".to_string()); // "direct" or "git"
    // Git fields
    let mut git_provider = use_signal(|| "github".to_string());
    let mut git_owner = use_signal(String::new);
    let mut git_repo = use_signal(String::new);
    let mut git_branch = use_signal(|| "main".to_string());
    let mut build_cmd = use_signal(String::new);
    let mut dest_dir = use_signal(String::new);
    let mut root_dir = use_signal(String::new);

    let mut deploying = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    if cred_list.is_empty() {
        return rsx! {
            Card { div { class: "p-6 text-fg-muted",
                "No Cloudflare credentials. "
                Link { to: crate::web::app::Route::CredentialForm {}, class: "text-brand underline", "Add one" }
                " first."
            }}
        };
    }

    let is_git = *mode.read() == "git";

    rsx! {
        Card { div { class: "p-6 space-y-4",
            p { class: "text-sm text-fg-muted",
                "Choose the deployment source. " span { class: "font-semibold text-fg", "This cannot be changed after creation." }
            }

            div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                FormField { label: "Cloudflare Credential",
                    select { class: "input", value: "{cred_id}", oninput: move |evt| cred_id.set(evt.value()),
                        for c in &cred_list { option { value: "{c.id}", "{c.name}" } }
                    }
                }
                FormField { label: "Deployment Source",
                    select { class: "input", value: "{mode}", oninput: move |evt| mode.set(evt.value()),
                        option { value: "direct", "Direct Upload (Wrangler CLI)" }
                        option { value: "git", "Git Repository (GitHub / GitLab)" }
                    }
                }
            }

            if is_git {
                p { class: "text-sm text-fg-muted",
                    "The GitHub/GitLab integration must be "
                    a { href: "https://dash.cloudflare.com/?to=/:account/pages", target: "_blank", class: "text-brand underline", "authorized in Cloudflare" }
                    " first."
                }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Provider",
                        select { class: "input", value: "{git_provider}", oninput: move |evt| git_provider.set(evt.value()),
                            option { value: "github", "GitHub" } option { value: "gitlab", "GitLab" }
                        }
                    }
                    FormField { label: "Branch",
                        input { class: "input", r#type: "text", value: "{git_branch}", placeholder: "main",
                            oninput: move |evt| git_branch.set(evt.value()) }
                    }
                }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Owner",
                        input { class: "input", r#type: "text", required: true, placeholder: "my-org",
                            value: "{git_owner}", oninput: move |evt| git_owner.set(evt.value()) }
                    }
                    FormField { label: "Repository",
                        input { class: "input", r#type: "text", required: true, placeholder: "my-site",
                            value: "{git_repo}", oninput: move |evt| git_repo.set(evt.value()) }
                    }
                }
                SectionHeading { "Build Settings (optional)" }
                div { class: "grid grid-cols-1 md:grid-cols-3 gap-4",
                    FormField { label: "Build Command",
                        input { class: "input font-mono text-sm", r#type: "text", placeholder: "npm run build",
                            value: "{build_cmd}", oninput: move |evt| build_cmd.set(evt.value()) }
                    }
                    FormField { label: "Output Directory",
                        input { class: "input font-mono text-sm", r#type: "text", placeholder: "dist",
                            value: "{dest_dir}", oninput: move |evt| dest_dir.set(evt.value()) }
                    }
                    FormField { label: "Root Directory",
                        input { class: "input font-mono text-sm", r#type: "text", placeholder: "/",
                            value: "{root_dir}", oninput: move |evt| root_dir.set(evt.value()) }
                    }
                }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-danger text-sm", "{err}" }
            }

            Button { variant: ButtonVariant::Primary,
                disabled: *deploying.read() || (is_git && (git_owner.read().is_empty() || git_repo.read().is_empty())),
                onclick: {
                    let wid = webspace_id;
                    let cid_str = cred_id.read().clone();
                    let is_git = is_git;
                    let gp = git_provider.read().clone();
                    let go = git_owner.read().clone();
                    let gr = git_repo.read().clone();
                    let gb = git_branch.read().clone();
                    let bc = build_cmd.read().clone();
                    let dd = dest_dir.read().clone();
                    let rd = root_dir.read().clone();
                    move |_| {
                        let cid_str = cid_str.clone();
                        let gp = gp.clone(); let go = go.clone(); let gr = gr.clone();
                        let gb = gb.clone(); let bc = bc.clone(); let dd = dd.clone(); let rd = rd.clone();
                        deploying.set(true); error.set(None);
                        spawn(async move {
                            if let Ok(cid) = uuid::Uuid::parse_str(&cid_str) {
                                // Step 1: create project
                                match deploy_pages_project(WebspaceDeployPagesInput { id: wid, credential_id: cid }).await {
                                    Ok(_) => {
                                        // Step 2: if git, connect repo
                                        if is_git {
                                            if let Err(e) = connect_git_repo(WebspaceConnectGitInput {
                                                id: wid,
                                                provider: gp,
                                                owner: go,
                                                repo_name: gr,
                                                production_branch: gb,
                                                build_command: bc,
                                                destination_dir: dd,
                                                root_dir: rd,
                                            }).await {
                                                error.set(Some(format!("Project created but git connection failed: {e}")));
                                                deploying.set(false);
                                                refresh += 1;
                                                return;
                                            }
                                        }
                                        refresh += 1;
                                    }
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                            }
                            deploying.set(false);
                        });
                    }
                },
                if *deploying.read() { "Creating..." } else if is_git { "Create with Git" } else { "Create with Direct Upload" }
            }
        }}
    }
}

/// Display for a git-connected Pages project.
#[component]
fn GitSourceDisplay(
    git_source: GitRepoInfo,
    build_config: Option<BuildConfigInfo>,
    pages_subdomain: Option<String>,
) -> Element {
    let repo_url = match git_source.provider.as_str() {
        "github" => format!(
            "https://github.com/{}/{}",
            git_source.owner, git_source.repo
        ),
        "gitlab" => format!(
            "https://gitlab.com/{}/{}",
            git_source.owner, git_source.repo
        ),
        _ => format!("{}/{}", git_source.owner, git_source.repo),
    };

    rsx! {
        Card { div { class: "p-6 space-y-3",
            div { class: "flex items-center gap-2",
                Badge { variant: BadgeVariant::Info, "Git" }
                span { class: "font-mono text-sm", "{git_source.provider}" }
            }
            div { class: "flex items-center gap-2",
                span { class: "text-sm text-fg-muted", "Repository:" }
                a { href: "{repo_url}", target: "_blank", class: "font-mono text-sm text-brand underline", "{git_source.owner}/{git_source.repo}" }
            }
            div { class: "flex items-center gap-2",
                span { class: "text-sm text-fg-muted", "Branch:" }
                span { class: "font-mono text-sm", "{git_source.production_branch}" }
            }
            if let Some(bc) = &build_config {
                if bc.build_command.is_some() || bc.destination_dir.is_some() {
                    div { class: "border-t border-line-soft pt-3 mt-3",
                        div { class: "text-sm text-fg-muted mb-1", "Build Settings" }
                        if let Some(ref cmd) = bc.build_command {
                            div { span { class: "text-sm text-fg-muted", "Command: " } span { class: "font-mono text-sm bg-surface-2 px-2 py-0.5 rounded", "{cmd}" } }
                        }
                        if let Some(ref dir) = bc.destination_dir {
                            div { span { class: "text-sm text-fg-muted", "Output: " } span { class: "font-mono text-sm", "{dir}" } }
                        }
                        if let Some(ref root) = bc.root_dir {
                            div { span { class: "text-sm text-fg-muted", "Root: " } span { class: "font-mono text-sm", "{root}" } }
                        }
                    }
                }
            }
            if let Some(ref sub) = pages_subdomain {
                div { span { class: "text-sm text-fg-muted", "Preview: " }
                    a { href: "https://{sub}", target: "_blank", class: "font-mono text-sm text-brand underline", "https://{sub}" } }
            }
        }}
    }
}

/// Display for a direct-upload Pages project with deploy API instructions.
#[component]
fn DirectUploadDisplay(
    webspace_id: Uuid,
    project_name: String,
    production_branch: String,
    pages_subdomain: Option<String>,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let ws_id = webspace_id.to_string();

    let mut branch_input = use_signal(move || production_branch.clone());
    let mut saving_branch = use_signal(|| false);
    let mut branch_msg = use_signal(|| None::<String>);

    rsx! {
        Card { div { class: "p-6 space-y-4",
            div { class: "flex items-center gap-2",
                Badge { variant: BadgeVariant::Accent, "Direct Upload" }
                span { class: "font-mono text-sm text-fg-muted", "{project_name}" }
            }

            // Production branch setting
            div {
                div { class: "text-sm font-medium mb-1", "Production Branch" }
                div { class: "text-sm text-fg-muted mb-2",
                    "Deployments to this branch go live. Other branches create preview deployments."
                }
                div { class: "flex items-end gap-3",
                    FormField { label: "Branch",
                        input { class: "input w-48 font-mono text-sm", r#type: "text",
                            value: "{branch_input}",
                            oninput: move |evt| branch_input.set(evt.value()),
                        }
                    }
                    Button {
                        variant: ButtonVariant::Secondary,
                        disabled: *saving_branch.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let b = branch_input.read().clone();
                                saving_branch.set(true);
                                branch_msg.set(None);
                                spawn(async move {
                                    match update_production_branch(WebspaceSetProductionBranchInput { id: wid, branch: b }).await {
                                        Ok(()) => {
                                            refresh += 1;
                                        }
                                        Err(e) => {
                                            branch_msg.set(Some(format!("Error: {e}")));
                                            saving_branch.set(false);
                                        }
                                    }
                                });
                            }
                        },
                        if *saving_branch.read() { "Saving..." } else { "Update Branch" }
                    }
                    if let Some(msg) = &*branch_msg.read() {
                        span { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }
            }

            // Deploy instructions
            div { class: "border-t border-line-soft pt-4",
                div { class: "text-sm font-medium mb-1", "Deploy" }
                div { class: "text-sm text-fg-muted mb-2",
                    "Create a deploy token at " span { class: "font-semibold", "Tokens → Create Token → Kind: Deploy" }
                    ", then use the CLI to upload your build output."
                }

                div { class: "text-sm text-fg-muted mb-1", "Install:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all mb-3",
                    "cargo install --git https://git.plan.ai/plan-ai/mac-mgmt web-agency-upload"
                }

                div { class: "text-sm text-fg-muted mb-1", "Deploy:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all mb-3 whitespace-pre",
                    "export WEB_AGENCY_TOKEN=your-deploy-token\nexport WEB_AGENCY_URL=https://your-server.example.com\nweb-agency-upload ./dist"
                }

                div { class: "text-sm text-fg-muted mb-1", "Or with explicit arguments:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all mb-3 whitespace-pre",
                    "web-agency-upload ./dist \\\n  --token YOUR_TOKEN \\\n  --url https://your-server.example.com \\\n  --webspace-id {ws_id}"
                }

                div { class: "text-sm text-fg-muted mb-1", "Deploy to a preview branch:" }
                div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded select-all",
                    "web-agency-upload ./dist --branch preview"
                }

                div { class: "mt-3",
                    Link {
                        to: crate::web::app::Route::DocPage { slug: "upload-cli".into() },
                        class: "text-sm text-brand underline",
                        "Full CLI documentation →"
                    }
                }
            }

            if let Some(ref sub) = pages_subdomain {
                div { class: "border-t border-line-soft pt-4",
                    span { class: "text-sm text-fg-muted", "Preview: " }
                    a { href: "https://{sub}", target: "_blank", class: "font-mono text-sm text-brand underline", "https://{sub}" }
                }
            }
        }}
    }
}

/// Recent deployments table.
#[component]
fn DeploymentsSection(webspace_id: Uuid) -> Element {
    let deploys = use_server_future(move || {
        let wid = webspace_id;
        async move { list_deployments(WebspaceDeploymentsInput { id: wid }).await }
    })?;

    let rows = match &*deploys.read() {
        Some(Ok(r)) => r.clone(),
        Some(Err(e)) => {
            return rsx! { Card { div { class: "p-4 text-danger text-sm", "Error: {e}" } } };
        }
        None => return rsx! { Card { div { class: "p-4 text-fg-muted text-sm", "Loading..." } } },
    };

    rsx! {
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Status" } Th { "Size" } Th { "Created" } Th { "Error" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "4", "No deployments yet" } }
                        }
                        for row in &rows {
                            tr {
                                Td {
                                    match row.status.as_str() {
                                        "success" => rsx! { Badge { variant: BadgeVariant::Success, "Success" } },
                                        "failed" => rsx! { Badge { variant: BadgeVariant::Danger, "Failed" } },
                                        "deploying" | "uploading" => rsx! { Badge { variant: BadgeVariant::Warn, "{row.status}" } },
                                        "pending" => rsx! { Badge { "Pending" } },
                                        _ => rsx! { Badge { "{row.status}" } },
                                    }
                                }
                                TdMuted {
                                    {row.tarball_size.map(|s| {
                                        if s > 1_048_576 { format!("{:.1} MB", s as f64 / 1_048_576.0) }
                                        else if s > 1024 { format!("{:.0} KB", s as f64 / 1024.0) }
                                        else { format!("{s} B") }
                                    }).unwrap_or_else(|| "-".into())}
                                }
                                TdMuted { "{row.created_at}" }
                                Td {
                                    if let Some(err) = &row.error_message {
                                        span { class: "text-sm text-danger", "{err}" }
                                    } else {
                                        span { class: "text-fg-muted", "-" }
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

/// Inline deploy-token creation (org admin only).
#[component]
fn DeployTokenSection(webspace_id: Uuid, organization_id: Uuid) -> Element {
    let mut tokens = use_server_future(move || async move {
        list_webspace_tokens(webspace_id, organization_id).await
    })?;
    let mut creating = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut created = use_signal(|| None::<String>);

    let rows = match &*tokens.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        Card { div { class: "p-4 space-y-3",
            if let Some(tok) = &*created.read() {
                TokenReveal { value: tok.clone(), label: "Deploy token".to_string() }
            }
            TokenCreateForm {
                submit_label: "Create Deploy Token".to_string(),
                submitting: *creating.read(),
                on_submit: move |input: TokenCreateInput| {
                    let wid = webspace_id;
                    let oid = organization_id;
                    creating.set(true);
                    error.set(None);
                    spawn(async move {
                        match create_deploy_token(wid, oid, input.label, input.expires_in_secs).await {
                            Ok(r) => { created.set(Some(r.token)); tokens.restart(); }
                            Err(e) => error.set(Some(format!("{e}"))),
                        }
                        creating.set(false);
                    });
                },
            }
            if let Some(err) = &*error.read() {
                div { class: "text-danger text-sm", "{err}" }
            }
            if !rows.is_empty() {
                TokenTable {
                    rows: rows.iter().map(|t| TokenRow {
                        id: t.id.to_string(),
                        label: t.label.clone(),
                        kind: None,
                        scope: None,
                        scope_href: None,
                        revoked: t.revoked,
                        expired: t.expired,
                        created: t.created_at.clone(),
                        expires: t.expires_at.clone(),
                    }).collect::<Vec<_>>(),
                    show_expires: true,
                    on_revoke: move |id: String| {
                        let oid = organization_id;
                        spawn(async move {
                            if let Ok(tid) = id.parse::<Uuid>() {
                                if revoke_webspace_token(tid, oid).await.is_ok() {
                                    tokens.restart();
                                }
                            }
                        });
                    },
                }
            }
        }}
    }
}

// ── Move webspace ──────────────────────────────────────────────────

#[server]
async fn list_move_target_orgs() -> Result<Vec<crate::web::user::OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_write_orgs(&user, &pool).await
}

// ── Delete webspace ─────────────────────────────────────────────────
// delete_webspace now lives in the shared api_mcp layer (imported below).

#[component]
fn MoveWebspaceSection(webspace_id: Uuid, current_org_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let orgs = use_server_future(list_move_target_orgs)?;
    let mut selected_org = use_signal(String::new);
    let mut moving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let org_list = match &*orgs.read() {
        Some(Ok(list)) => list.clone(),
        _ => vec![],
    };

    let targets: Vec<_> = org_list
        .into_iter()
        .filter(|o| o.id != current_org_id)
        .collect();
    if targets.is_empty() {
        return rsx! {};
    }

    rsx! {
        Card {
            div { class: "p-4 flex items-center justify-between gap-4",
                div {
                    div { class: "font-medium text-danger", "Move to another organization" }
                    div { class: "text-sm text-fg-muted", "Transfers this webspace to a different organization." }
                }
                div { class: "flex items-center gap-2",
                    select {
                        class: "input w-48",
                        value: "{selected_org}",
                        onchange: move |evt| selected_org.set(evt.value()),
                        option { value: "", "Select org..." }
                        for org in &targets {
                            option { value: "{org.id}", "{org.name}" }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Danger,
                        disabled: selected_org.read().is_empty() || *moving.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let target = selected_org.read().clone();
                                if let Ok(tid) = Uuid::parse_str(&target) {
                                    moving.set(true);
                                    error.set(None);
                                    spawn(async move {
                                        match move_webspace(WebspaceMoveInput { id: wid, target_org_id: tid }).await {
                                            Ok(()) => {
                                                refresh += 1;
                                            }
                                            Err(e) => {
                                                error.set(Some(format!("{e}")));
                                                moving.set(false);
                                            }
                                        }
                                    });
                                }
                            }
                        },
                        if *moving.read() { "Moving..." } else { "Move Webspace" }
                    }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "px-4 pb-4 text-danger text-sm", "{err}" }
            }
        }
    }
}

#[component]
fn DeleteWebspaceSection(webspace_id: Uuid) -> Element {
    let mut deleting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-4 flex items-center justify-between",
                div {
                    div { class: "text-sm font-medium", "Delete this webspace" }
                    div { class: "text-sm text-fg-muted", "All domain bindings and deployments will be removed." }
                }
                Button {
                    variant: ButtonVariant::Danger,
                    disabled: *deleting.read(),
                    onclick: {
                        let wid = webspace_id;
                        move |_| {
                            deleting.set(true);
                            error.set(None);
                            spawn(async move {
                                match delete_webspace(WebspaceDeleteInput { id: wid }).await {
                                    Ok(()) => { navigator().push(crate::web::app::Route::WebspaceList {}); }
                                    Err(e) => {
                                        error.set(Some(format!("{e}")));
                                        deleting.set(false);
                                    }
                                }
                            });
                        }
                    },
                    if *deleting.read() { "Deleting..." } else { "Delete Webspace" }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "px-4 pb-4 text-danger text-sm", "{err}" }
            }
        }
    }
}

// ── Webspace settings (name, upstream URL) ──────────────────────────

#[component]
fn WebspaceSettingsSection(
    webspace_id: Uuid,
    current_name: String,
    hosting_type: String,
    current_relay_url: Option<String>,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut name = use_signal(move || current_name.clone());
    let mut relay_url = use_signal(move || current_relay_url.clone().unwrap_or_default());
    let mut saving = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    let is_relay_or_tunnel = hosting_type == "relay" || hosting_type == "tunnel";

    rsx! {
        Card {
            div { class: "p-4",
                div { class: "flex items-end gap-3 flex-wrap",
                    FormField { label: "Name",
                        input {
                            class: "input w-64",
                            r#type: "text",
                            value: "{name}",
                            oninput: move |evt| name.set(evt.value()),
                        }
                    }

                    if is_relay_or_tunnel {
                        FormField { label: "Upstream URL",
                            input {
                                class: "input w-80 font-mono",
                                r#type: "url",
                                placeholder: "https://backend.example.com",
                                value: "{relay_url}",
                                oninput: move |evt| relay_url.set(evt.value()),
                            }
                        }
                    }

                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *saving.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let n = name.read().clone();
                                let url = if is_relay_or_tunnel {
                                    let u = relay_url.read().clone();
                                    if u.is_empty() { None } else { Some(u) }
                                } else {
                                    None
                                };
                                saving.set(true);
                                message.set(None);
                                spawn(async move {
                                    let clear_relay_url = Some(url.is_none());
                                    match update_webspace_settings(WebspaceUpdateInput {
                                        id: wid,
                                        name: Some(n),
                                        path_prefix: None,
                                        relay_url: url,
                                        clear_relay_url,
                                        auth_mode: None,
                                        auth_basic_list_id: None,
                                        clear_auth_basic_list: None,
                                    }).await {
                                        Ok(()) => {
                                            message.set(Some("Saved".into()));
                                            refresh += 1;
                                        }
                                        Err(e) => message.set(Some(format!("Error: {e}"))),
                                    }
                                    saving.set(false);
                                });
                            }
                        },
                        if *saving.read() { "Saving..." } else { "Save" }
                    }

                    if let Some(msg) = &*message.read() {
                        span { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }
            }
        }
    }
}

// ── Auth settings ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BasicAuthListOption {
    id: Uuid,
    name: String,
}

#[server]
async fn load_basic_auth_lists(org_id: Uuid) -> Result<Vec<BasicAuthListOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM basic_auth_lists WHERE organization_id = $1 ORDER BY name",
    )
    .bind(org_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| BasicAuthListOption { id, name })
        .collect())
}

#[component]
fn AuthSettingsSection(
    webspace_id: Uuid,
    organization_id: Uuid,
    current_mode: String,
    current_list_name: Option<String>,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let lists = use_server_future(move || {
        let oid = organization_id;
        async move { load_basic_auth_lists(oid).await }
    })?;
    let basic_auth_lists = match &*lists.read() {
        Some(Ok(l)) => l.clone(),
        _ => vec![],
    };

    let mut auth_mode = use_signal(move || current_mode.clone());
    let mut basic_list_id = use_signal(|| {
        basic_auth_lists
            .first()
            .map(|b| b.id.to_string())
            .unwrap_or_default()
    });
    let mut saving = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-4 space-y-4",
                div { class: "flex items-end gap-3 flex-wrap",
                    FormField { label: "Auth Mode",
                        select {
                            class: "input w-48",
                            value: "{auth_mode}",
                            oninput: move |evt| auth_mode.set(evt.value()),
                            option { value: "none", "None" }
                            option { value: "oidc", "OIDC (org members)" }
                            option { value: "basic", "HTTP Basic" }
                        }
                    }

                    if *auth_mode.read() == "basic" {
                        FormField { label: "Basic Auth List",
                            select {
                                class: "input w-48",
                                value: "{basic_list_id}",
                                oninput: move |evt| basic_list_id.set(evt.value()),
                                if basic_auth_lists.is_empty() {
                                    option { value: "", "No lists — create one first" }
                                }
                                for b in &basic_auth_lists {
                                    option { value: "{b.id}", "{b.name}" }
                                }
                            }
                        }
                    }

                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *saving.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let mode = auth_mode.read().clone();
                                let list_id = if mode == "basic" {
                                    uuid::Uuid::parse_str(&basic_list_id.read()).ok()
                                } else {
                                    None
                                };
                                saving.set(true);
                                message.set(None);
                                spawn(async move {
                                    let clear_auth_basic_list = Some(list_id.is_none());
                                    match update_webspace_settings(WebspaceUpdateInput {
                                        id: wid,
                                        name: None,
                                        path_prefix: None,
                                        relay_url: None,
                                        clear_relay_url: None,
                                        auth_mode: Some(mode),
                                        auth_basic_list_id: list_id,
                                        clear_auth_basic_list,
                                    }).await {
                                        Ok(()) => {
                                            message.set(Some("Saved".into()));
                                            refresh += 1;
                                        }
                                        Err(e) => message.set(Some(format!("Error: {e}"))),
                                    }
                                    saving.set(false);
                                });
                            }
                        },
                        if *saving.read() { "Saving..." } else { "Save" }
                    }

                    if let Some(msg) = &*message.read() {
                        span { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }

                if *auth_mode.read() == "oidc" {
                    div { class: "text-sm text-fg-muted",
                        "Members of this webspace's organization will have access after logging in via the agency."
                    }
                }
            }
        }
    }
}
