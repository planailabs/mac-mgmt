pub use plan_ai_auth::WebUser;

/// Dioxus-specific extensions for WebUser (wraps plan-ai-auth's string errors
/// into ServerFnError).
pub trait WebUserExt {
    fn require_admin(&self) -> Result<(), dioxus::prelude::ServerFnError>;
    /// Require read access to an organization. Admins always pass.
    fn require_org_read(&self, org_id: &uuid::Uuid) -> Result<(), dioxus::prelude::ServerFnError>;
}

impl WebUserExt for WebUser {
    fn require_admin(&self) -> Result<(), dioxus::prelude::ServerFnError> {
        self.require_admin_str()
            .map_err(|e| dioxus::prelude::ServerFnError::new(e))
    }

    fn require_org_read(&self, org_id: &uuid::Uuid) -> Result<(), dioxus::prelude::ServerFnError> {
        if self.is_admin || self.org_ids().contains(org_id) {
            Ok(())
        } else {
            Err(dioxus::prelude::ServerFnError::new("access denied"))
        }
    }
}

/// Extract the current authenticated user from the request extensions.
#[cfg(feature = "server")]
pub async fn current_user() -> Result<WebUser, dioxus::prelude::ServerFnError> {
    use dioxus::fullstack::axum::extract::Extension;
    let Extension(user): Extension<WebUser> = dioxus::fullstack::extract()
        .await
        .map_err(|_| dioxus::prelude::ServerFnError::new("not authenticated"))?;
    Ok(user)
}

// ── plan-ai-api-mcp bridge ──────────────────────────────────────────────
//
// Turn a web-session `WebUser` into a framework `Principal`, and map framework
// `ApiError`s back to `ServerFnError`. These let the macro-generated Dioxus
// `#[server]` wrappers delegate to shared endpoint handlers.

/// Build a framework `Principal` from the authenticated web user.
#[cfg(feature = "server")]
pub fn principal_from(user: &WebUser) -> plan_ai_api_mcp::Principal {
    if user.is_admin {
        plan_ai_api_mcp::Principal::admin(user.email.clone())
    } else {
        plan_ai_api_mcp::Principal::scoped(
            user.email.clone(),
            user.org_ids().into_iter().collect(),
            user.write_org_ids().into_iter().collect(),
        )
    }
}

/// Map a framework `ApiError` to a Dioxus `ServerFnError`.
#[cfg(feature = "server")]
pub fn to_serverfn(err: plan_ai_api_mcp::ApiError) -> dioxus::prelude::ServerFnError {
    dioxus::prelude::ServerFnError::new(err.to_string())
}

// ── Server-side access helpers ──────────────────────────────────────────
//
// These query the database and check org membership in one call, removing
// duplicated access-control boilerplate from individual server functions.

#[cfg(feature = "server")]
use dioxus::prelude::ServerFnError;
#[cfg(feature = "server")]
use sqlx::PgPool;
#[cfg(feature = "server")]
use uuid::Uuid;

/// Shared (id, name) pair returned by org-listing helpers and used by
/// form dropdowns across the app.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrgOption {
    pub id: uuid::Uuid,
    pub name: String,
}

/// List organizations visible to the user (any role). Admins see all.
#[cfg(feature = "server")]
pub async fn list_user_orgs(
    user: &WebUser,
    pool: &PgPool,
) -> Result<Vec<OrgOption>, ServerFnError> {
    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM organizations ORDER BY name")
            .fetch_all(pool)
            .await
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT o.id, o.name FROM organizations o \
             JOIN organization_members om ON om.organization_id = o.id \
             WHERE om.user_id = $1 ORDER BY o.name",
        )
        .bind(user.id)
        .fetch_all(pool)
        .await
    }
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, name)| OrgOption { id, name })
        .collect())
}

/// List organizations where the user has write or admin role. Admins see all.
#[cfg(feature = "server")]
pub async fn list_user_write_orgs(
    user: &WebUser,
    pool: &PgPool,
) -> Result<Vec<OrgOption>, ServerFnError> {
    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM organizations ORDER BY name")
            .fetch_all(pool)
            .await
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT o.id, o.name FROM organizations o \
             JOIN organization_members om ON om.organization_id = o.id \
             WHERE om.user_id = $1 AND om.role IN ('admin', 'write') ORDER BY o.name",
        )
        .bind(user.id)
        .fetch_all(pool)
        .await
    }
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, name)| OrgOption { id, name })
        .collect())
}
