use plan_ai_auth::{OrgMembership, WebUser};
use std::sync::Arc;
use uuid::Uuid;

use crate::config;

// Re-export items used by main.rs router setup.
pub use plan_ai_auth::{
    IMPERSONATE_COOKIE, build_auth_layers, login_page, logout_handler, require_auth,
    set_user_resolver,
};

/// Application-specific user resolver backed by PostgreSQL.
pub struct PgUserResolver {
    pool: sqlx::PgPool,
    admin_emails: Vec<String>,
}

impl PgUserResolver {
    pub fn new(pool: sqlx::PgPool, admin_emails: Vec<String>) -> Arc<Self> {
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

        // Upsert user: create on first login, update name on subsequent logins.
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
            let org_id =
                sqlx::query_scalar::<_, Uuid>("SELECT id FROM organizations WHERE name = $1")
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

        self.with_memberships(user).await
    }

    async fn load_user_by_id(&self, id: Uuid) -> Result<Option<WebUser>, anyhow::Error> {
        let user = sqlx::query_as::<_, (Uuid, String, String, bool)>(
            "SELECT id, email, name, is_admin FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        match user {
            Some(user) => Ok(Some(self.with_memberships(user).await?)),
            None => Ok(None),
        }
    }
}

impl PgUserResolver {
    /// The user a web session with this email resolves to, without creating
    /// or changing anything: for credentials that vouch for an email but must
    /// not enroll users (Zitadel bearers on the management API). `None` when
    /// no such user exists. An `admin_emails` address counts as admin, as
    /// [`resolve_user`](plan_ai_auth::UserResolver::resolve_user) would make it.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<WebUser>, anyhow::Error> {
        let user = sqlx::query_as::<_, (Uuid, String, String, bool)>(
            "SELECT id, email, name, is_admin FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        let Some(mut user) = user else {
            return Ok(None);
        };
        user.3 |= self.admin_emails.iter().any(|a| a == email);
        Ok(Some(self.with_memberships(user).await?))
    }

    /// `WebUser` for a `users` row, with its organization memberships.
    async fn with_memberships(
        &self,
        (id, email, name, is_admin): (Uuid, String, String, bool),
    ) -> Result<WebUser, anyhow::Error> {
        let org_memberships = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT organization_id, role FROM organization_members WHERE user_id = $1",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(org_id, role)| OrgMembership::new(org_id, role))
        .collect();
        Ok(WebUser {
            id,
            email,
            name,
            is_admin,
            org_memberships,
            impersonating_from: None,
        })
    }
}

/// The configured `[auth].admin_emails`.
pub fn admin_emails() -> Vec<String> {
    config::config()
        .auth
        .as_ref()
        .map(|a| a.admin_emails.clone())
        .unwrap_or_default()
}

/// Install the user resolver at startup.
pub fn install_resolver(pool: sqlx::PgPool) {
    set_user_resolver(PgUserResolver::new(pool, admin_emails()));
}

// ── Impersonation routes (app-specific) ─────────────────────────────────

use dioxus::fullstack::axum::{self as axum, body::Body, extract::Request, response::IntoResponse};

/// Start admin impersonation. Sets the `impersonate_user_id` cookie HttpOnly,
/// Secure, and SameSite=Strict so it can only be set/cleared via this server
/// endpoint and isn't readable from JS.
pub async fn start_impersonation(
    axum::extract::Path(target_id): axum::extract::Path<Uuid>,
    request: Request<Body>,
) -> impl IntoResponse {
    let user = match request.extensions().get::<WebUser>().cloned() {
        Some(u) => u,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "no session").into_response(),
    };
    if !(user.is_admin || user.impersonating_from.is_some()) {
        return (axum::http::StatusCode::FORBIDDEN, "admin required").into_response();
    }
    let real_admin = user.impersonating_from.unwrap_or(user.id);
    tracing::info!(real_admin = %real_admin, target = %target_id, "impersonation started");

    let cookie =
        format!("{IMPERSONATE_COOKIE}={target_id}; Path=/; HttpOnly; Secure; SameSite=Strict");
    let mut response = (axum::http::StatusCode::NO_CONTENT, ()).into_response();
    if let Ok(value) = axum::http::HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(axum::http::header::SET_COOKIE, value);
    }
    response
}

/// Stop admin impersonation by clearing the cookie.
pub async fn stop_impersonation(request: Request<Body>) -> impl IntoResponse {
    let user = match request.extensions().get::<WebUser>().cloned() {
        Some(u) => u,
        None => return (axum::http::StatusCode::UNAUTHORIZED, "no session").into_response(),
    };
    let real_admin = user.impersonating_from.unwrap_or(user.id);
    tracing::info!(real_admin = %real_admin, "impersonation stopped");

    let cookie =
        format!("{IMPERSONATE_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0");
    let mut response = (axum::http::StatusCode::NO_CONTENT, ()).into_response();
    if let Ok(value) = axum::http::HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(axum::http::header::SET_COOKIE, value);
    }
    response
}
