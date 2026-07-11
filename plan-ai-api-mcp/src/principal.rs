//! Auth-neutral authorization principal.
//!
//! Both a web session user and a Bearer API token resolve to a [`Principal`],
//! so the same endpoint handler can authorize a UI call, an HTTP-API call, and
//! an MCP tool call through one code path.

use std::collections::HashSet;

use uuid::Uuid;

use crate::error::ApiError;

/// The set of organizations a principal may act on.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OrgSet {
    /// Every organization (global admin).
    All,
    /// Only the listed organizations.
    Only(HashSet<Uuid>),
}

impl OrgSet {
    pub fn contains(&self, org: &Uuid) -> bool {
        match self {
            OrgSet::All => true,
            OrgSet::Only(set) => set.contains(org),
        }
    }

    /// `None` when the principal may see every org (no SQL filter needed);
    /// `Some(ids)` to filter with `WHERE organization_id = ANY($1)`.
    pub fn as_filter(&self) -> Option<Vec<Uuid>> {
        match self {
            OrgSet::All => None,
            OrgSet::Only(set) => Some(set.iter().copied().collect()),
        }
    }
}

/// A resolved caller. Authorization decisions read only from this type.
/// Serializable so callers may persist an authz snapshot (e.g. a durable job
/// queue resuming work as the original caller after a restart).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Principal {
    /// Global super-user: bypasses org checks entirely.
    pub admin: bool,
    /// Organizations the principal may read.
    pub read_orgs: OrgSet,
    /// Organizations the principal may mutate.
    pub write_orgs: OrgSet,
    /// Opaque per-resource scopes (e.g. a deploy token's `webspace_id`).
    /// The framework never interprets this; endpoints may.
    pub scopes: serde_json::Value,
    /// Stable identifier for audit logging (email, token id, …).
    pub subject: String,
}

impl Principal {
    /// A global-admin principal (all orgs, read + write).
    pub fn admin(subject: impl Into<String>) -> Self {
        Self {
            admin: true,
            read_orgs: OrgSet::All,
            write_orgs: OrgSet::All,
            scopes: serde_json::Value::Null,
            subject: subject.into(),
        }
    }

    /// A principal scoped to a fixed set of orgs.
    pub fn scoped(subject: impl Into<String>, read: HashSet<Uuid>, write: HashSet<Uuid>) -> Self {
        Self {
            admin: false,
            read_orgs: OrgSet::Only(read),
            write_orgs: OrgSet::Only(write),
            scopes: serde_json::Value::Null,
            subject: subject.into(),
        }
    }

    pub fn can_read(&self, org: &Uuid) -> bool {
        self.admin || self.read_orgs.contains(org)
    }

    pub fn can_write(&self, org: &Uuid) -> bool {
        self.admin || self.write_orgs.contains(org)
    }

    /// SQL org filter for list queries: `None` = admin (no filter), `Some` = restrict.
    pub fn read_filter(&self) -> Option<Vec<Uuid>> {
        if self.admin {
            None
        } else {
            self.read_orgs.as_filter()
        }
    }

    pub fn require_admin(&self) -> Result<(), ApiError> {
        if self.admin {
            Ok(())
        } else {
            Err(ApiError::Forbidden("admin required".into()))
        }
    }

    pub fn require_read(&self, org: &Uuid) -> Result<(), ApiError> {
        if self.can_read(org) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!("no read access to org {org}")))
        }
    }

    pub fn require_write(&self, org: &Uuid) -> Result<(), ApiError> {
        if self.can_write(org) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!("no write access to org {org}")))
        }
    }
}
