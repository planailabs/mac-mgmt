pub use plan_ai_auth::WebUser;

/// Dioxus-specific extension methods for `WebUser`.
#[cfg(feature = "server")]
#[async_trait::async_trait]
pub trait WebUserExt {
    fn require_admin(&self) -> Result<(), dioxus::prelude::ServerFnError>;
    fn require_org_admin(&self, org_id: &uuid::Uuid) -> Result<(), dioxus::prelude::ServerFnError>;

    async fn accessible_cluster_ids(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<Vec<uuid::Uuid>>, sqlx::Error>;

    async fn writable_cluster_ids(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<Vec<uuid::Uuid>>, sqlx::Error>;

    async fn require_cluster_write(
        &self,
        pool: &sqlx::PgPool,
        cluster_id: uuid::Uuid,
    ) -> Result<(), dioxus::prelude::ServerFnError>;

    async fn require_cluster_read(
        &self,
        pool: &sqlx::PgPool,
        cluster_id: uuid::Uuid,
    ) -> Result<(), dioxus::prelude::ServerFnError>;
}

#[cfg(feature = "server")]
#[async_trait::async_trait]
impl WebUserExt for WebUser {
    fn require_admin(&self) -> Result<(), dioxus::prelude::ServerFnError> {
        self.require_admin_str()
            .map_err(|e| dioxus::prelude::ServerFnError::new(e))
    }

    fn require_org_admin(&self, org_id: &uuid::Uuid) -> Result<(), dioxus::prelude::ServerFnError> {
        self.require_org_admin_str(org_id)
            .map_err(|e| dioxus::prelude::ServerFnError::new(e))
    }

    async fn accessible_cluster_ids(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<Vec<uuid::Uuid>>, sqlx::Error> {
        if self.is_admin {
            return Ok(None);
        }
        let org_ids = self.org_ids();
        if org_ids.is_empty() {
            return Ok(Some(vec![]));
        }
        let ids = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT DISTINCT oc.cluster_id \
             FROM organization_clusters oc \
             WHERE oc.organization_id = ANY($1)",
        )
        .bind(&org_ids)
        .fetch_all(pool)
        .await?;
        Ok(Some(ids))
    }

    async fn writable_cluster_ids(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<Vec<uuid::Uuid>>, sqlx::Error> {
        if self.is_admin {
            return Ok(None);
        }
        let org_ids = self.write_org_ids();
        if org_ids.is_empty() {
            return Ok(Some(vec![]));
        }
        let ids = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT DISTINCT oc.cluster_id \
             FROM organization_clusters oc \
             WHERE oc.organization_id = ANY($1)",
        )
        .bind(&org_ids)
        .fetch_all(pool)
        .await?;
        Ok(Some(ids))
    }

    async fn require_cluster_write(
        &self,
        pool: &sqlx::PgPool,
        cluster_id: uuid::Uuid,
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

    async fn require_cluster_read(
        &self,
        pool: &sqlx::PgPool,
        cluster_id: uuid::Uuid,
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
    let Extension(user): Extension<WebUser> = dioxus::fullstack::FullstackContext::extract()
        .await
        .map_err(|_| dioxus::prelude::ServerFnError::new("not authenticated"))?;
    Ok(user)
}
