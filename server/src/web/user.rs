use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lightweight user context extracted from the OIDC session and stored
/// in axum request extensions for use in Dioxus server functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub org_ids: Vec<Uuid>,
    /// If set, this user context is the result of admin impersonation.
    /// The value is the real admin's user ID.
    #[serde(default)]
    pub impersonating_from: Option<Uuid>,
}

impl WebUser {
    pub fn require_admin(&self) -> Result<(), dioxus::prelude::ServerFnError> {
        if !self.is_admin {
            return Err(dioxus::prelude::ServerFnError::new("admin access required"));
        }
        Ok(())
    }

    /// Returns the cluster IDs this user can access.
    /// `None` means all clusters (admin). `Some(ids)` for org-scoped users.
    #[cfg(feature = "server")]
    pub async fn accessible_cluster_ids(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<Vec<Uuid>>, sqlx::Error> {
        if self.is_admin {
            return Ok(None);
        }
        let ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT DISTINCT oc.cluster_id \
             FROM organization_clusters oc \
             WHERE oc.organization_id = ANY($1)",
        )
        .bind(&self.org_ids)
        .fetch_all(pool)
        .await?;
        Ok(Some(ids))
    }
}

/// Extract the current authenticated user from the request extensions.
/// Call this in `#[server]` functions to get the logged-in user.
#[cfg(feature = "server")]
pub async fn current_user() -> Result<WebUser, dioxus::prelude::ServerFnError> {
    use dioxus::fullstack::axum::extract::Extension;
    let Extension(user): Extension<WebUser> =
        dioxus::fullstack::extract()
            .await
            .map_err(|_| dioxus::prelude::ServerFnError::new("not authenticated"))?;
    Ok(user)
}
