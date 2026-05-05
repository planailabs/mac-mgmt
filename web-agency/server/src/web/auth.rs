//! OIDC auth integration for web-agency-server.
//!
//! Implements the `UserResolver` trait from `plan-ai-auth` to upsert users
//! into the web-agency database and resolve org memberships.

use plan_ai_auth::{OrgMembership, WebUser};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub use plan_ai_auth::{
    build_auth_layers, login_page, logout_handler, require_auth, set_user_resolver,
};

/// Web-agency-specific user resolver backed by PostgreSQL.
pub struct PgUserResolver {
    pool: PgPool,
    admin_emails: Vec<String>,
}

impl PgUserResolver {
    pub fn new(pool: PgPool, admin_emails: Vec<String>) -> Arc<Self> {
        Arc::new(Self { pool, admin_emails })
    }
}

#[async_trait::async_trait]
impl plan_ai_auth::UserResolver for PgUserResolver {
    async fn resolve_user(
        &self,
        email: &str,
        name: Option<&str>,
        _admin_emails: &[String],
        auto_join_orgs: &[String],
    ) -> Result<WebUser, anyhow::Error> {
        let is_admin_email = self.admin_emails.contains(&email.to_string());
        let display_name = name.unwrap_or("");

        let user = if is_admin_email {
            sqlx::query_as::<_, (Uuid, String, String, bool)>(
                "INSERT INTO users (email, name, is_admin) VALUES ($1, $2, true) \
                 ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name, is_admin = true \
                 RETURNING id, email, name, is_admin",
            )
            .bind(email)
            .bind(display_name)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (Uuid, String, String, bool)>(
                "INSERT INTO users (email, name) VALUES ($1, $2) \
                 ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name \
                 RETURNING id, email, name, is_admin",
            )
            .bind(email)
            .bind(display_name)
            .fetch_one(&self.pool)
            .await?
        };

        // Auto-join organizations for this provider (idempotent).
        for org_name in auto_join_orgs {
            let org_id = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM organizations WHERE name = $1",
            )
            .bind(org_name)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(org_id) = org_id {
                sqlx::query(
                    "INSERT INTO organization_members (organization_id, user_id, role) \
                     VALUES ($1, $2, 'read') ON CONFLICT DO NOTHING",
                )
                .bind(org_id)
                .bind(user.0)
                .execute(&self.pool)
                .await?;
            }
        }

        let org_memberships = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT organization_id, role FROM organization_members WHERE user_id = $1",
        )
        .bind(user.0)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(org_id, role)| OrgMembership { org_id, role })
        .collect();

        Ok(WebUser {
            id: user.0,
            email: user.1,
            name: user.2,
            is_admin: user.3,
            org_memberships,
            impersonating_from: None,
        })
    }

    async fn load_user_by_id(&self, id: Uuid) -> Result<Option<WebUser>, anyhow::Error> {
        let user = sqlx::query_as::<_, (Uuid, String, String, bool)>(
            "SELECT id, email, name, is_admin FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match user {
            Some(u) => {
                let org_memberships = sqlx::query_as::<_, (Uuid, String)>(
                    "SELECT organization_id, role FROM organization_members WHERE user_id = $1",
                )
                .bind(u.0)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|(org_id, role)| OrgMembership { org_id, role })
                .collect();

                Ok(Some(WebUser {
                    id: u.0,
                    email: u.1,
                    name: u.2,
                    is_admin: u.3,
                    org_memberships,
                    impersonating_from: None,
                }))
            }
            None => Ok(None),
        }
    }
}
