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

    /// Require global admin access. Returns a string error suitable for
    /// conversion into framework-specific error types.
    pub fn require_admin_str(&self) -> Result<(), &'static str> {
        if self.is_admin {
            Ok(())
        } else {
            Err("admin access required")
        }
    }

    /// Require org admin access. Returns a string error.
    pub fn require_org_admin_str(&self, org_id: &Uuid) -> Result<(), &'static str> {
        if self.is_org_admin(org_id) {
            Ok(())
        } else {
            Err("organization admin access required")
        }
    }
}
