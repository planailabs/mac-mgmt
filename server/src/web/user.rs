use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Organization membership with role information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgMembership {
    pub org_id: Uuid,
    pub role: String, // "admin", "write", "read"
}

/// Lightweight user context extracted from the OIDC session and stored
/// in axum request extensions for use in Dioxus server functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub org_memberships: Vec<OrgMembership>,
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

    /// All organization IDs this user belongs to (any role).
    pub fn org_ids(&self) -> Vec<Uuid> {
        self.org_memberships.iter().map(|m| m.org_id).collect()
    }

    /// Organization IDs where user has write or admin role.
    pub fn write_org_ids(&self) -> Vec<Uuid> {
        self.org_memberships
            .iter()
            .filter(|m| m.role == "admin" || m.role == "write")
            .map(|m| m.org_id)
            .collect()
    }

    /// Check if user is an admin of a specific organization.
    pub fn is_org_admin(&self, org_id: &Uuid) -> bool {
        self.is_admin
            || self
                .org_memberships
                .iter()
                .any(|m| m.org_id == *org_id && m.role == "admin")
    }

    /// Require that the user is an admin of the given organization (or a global admin).
    pub fn require_org_admin(&self, org_id: &Uuid) -> Result<(), dioxus::prelude::ServerFnError> {
        if !self.is_org_admin(org_id) {
            return Err(dioxus::prelude::ServerFnError::new(
                "organization admin access required",
            ));
        }
        Ok(())
    }

    /// Returns the cluster IDs this user can access (read).
    /// `None` means all clusters (admin). `Some(ids)` for org-scoped users.
    #[cfg(feature = "server")]
    pub async fn accessible_cluster_ids(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<Vec<Uuid>>, sqlx::Error> {
        if self.is_admin {
            return Ok(None);
        }
        let org_ids = self.org_ids();
        if org_ids.is_empty() {
            return Ok(Some(vec![]));
        }
        let ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT DISTINCT oc.cluster_id \
             FROM organization_clusters oc \
             WHERE oc.organization_id = ANY($1)",
        )
        .bind(&org_ids)
        .fetch_all(pool)
        .await?;
        Ok(Some(ids))
    }

    /// Returns the cluster IDs this user can write to (write or admin org role).
    /// `None` means all clusters (global admin). `Some(ids)` for org-scoped users.
    #[cfg(feature = "server")]
    pub async fn writable_cluster_ids(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<Vec<Uuid>>, sqlx::Error> {
        if self.is_admin {
            return Ok(None);
        }
        let org_ids = self.write_org_ids();
        if org_ids.is_empty() {
            return Ok(Some(vec![]));
        }
        let ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT DISTINCT oc.cluster_id \
             FROM organization_clusters oc \
             WHERE oc.organization_id = ANY($1)",
        )
        .bind(&org_ids)
        .fetch_all(pool)
        .await?;
        Ok(Some(ids))
    }

    /// Check write access for a specific cluster. Returns error if denied.
    #[cfg(feature = "server")]
    pub async fn require_cluster_write(
        &self,
        pool: &sqlx::PgPool,
        cluster_id: Uuid,
    ) -> Result<(), dioxus::prelude::ServerFnError> {
        if let Some(ids) = self
            .writable_cluster_ids(pool)
            .await
            .map_err(|e| dioxus::prelude::ServerFnError::new(e.to_string()))?
        {
            if !ids.contains(&cluster_id) {
                return Err(dioxus::prelude::ServerFnError::new("write access denied"));
            }
        }
        Ok(())
    }

    /// Check read access for a specific cluster. Returns error if denied.
    #[cfg(feature = "server")]
    pub async fn require_cluster_read(
        &self,
        pool: &sqlx::PgPool,
        cluster_id: Uuid,
    ) -> Result<(), dioxus::prelude::ServerFnError> {
        if let Some(ids) = self
            .accessible_cluster_ids(pool)
            .await
            .map_err(|e| dioxus::prelude::ServerFnError::new(e.to_string()))?
        {
            if !ids.contains(&cluster_id) {
                return Err(dioxus::prelude::ServerFnError::new("access denied"));
            }
        }
        Ok(())
    }
}

/// Extract the current authenticated user from the request extensions.
/// Call this in `#[server]` functions to get the logged-in user.
#[cfg(feature = "server")]
pub async fn current_user() -> Result<WebUser, dioxus::prelude::ServerFnError> {
    use dioxus::fullstack::axum::extract::Extension;
    let Extension(user): Extension<WebUser> = dioxus::fullstack::extract()
        .await
        .map_err(|_| dioxus::prelude::ServerFnError::new("not authenticated"))?;
    Ok(user)
}
