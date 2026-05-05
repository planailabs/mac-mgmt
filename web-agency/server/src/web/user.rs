pub use plan_ai_auth::{OrgMembership, WebUser};

/// Dioxus-specific extensions for WebUser (wraps plan-ai-auth's string errors
/// into ServerFnError).
pub trait WebUserExt {
    fn require_admin(&self) -> Result<(), dioxus::prelude::ServerFnError>;
    fn require_org_admin(&self, org_id: &uuid::Uuid) -> Result<(), dioxus::prelude::ServerFnError>;
}

impl WebUserExt for WebUser {
    fn require_admin(&self) -> Result<(), dioxus::prelude::ServerFnError> {
        self.require_admin_str()
            .map_err(|e| dioxus::prelude::ServerFnError::new(e))
    }

    fn require_org_admin(&self, org_id: &uuid::Uuid) -> Result<(), dioxus::prelude::ServerFnError> {
        self.require_org_admin_str(org_id)
            .map_err(|e| dioxus::prelude::ServerFnError::new(e))
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
