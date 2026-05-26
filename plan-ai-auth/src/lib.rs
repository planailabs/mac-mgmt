//! Shared OIDC authentication library for plan-ai projects.
//!
//! Provides multi-provider OIDC login, session management, user resolution,
//! and access control types shared between mac-mgmt-server and web-agency-server.
//!
//! # Features
//!
//! - **default** (no features): Only the types (`WebUser`, `OrgMembership`,
//!   `AuthConfig`, `OidcProviderConfig`). Safe for WASM builds.
//! - **server**: Full auth middleware, login/logout handlers, session cache.
//!   Requires axum + axum-oidc-client.

mod config;
mod types;

pub use config::{AuthConfig, OidcProviderConfig};
pub use types::{OrgMembership, WebUser};

#[cfg(feature = "server")]
mod middleware;

#[cfg(feature = "server")]
pub use middleware::{
    AUTH_PROVIDERS, IMPERSONATE_COOKIE, ProviderMeta, UserResolver, build_auth_layers, login_page,
    logout_handler, require_auth, set_user_resolver,
};

#[cfg(feature = "server")]
pub use axum_oidc_client::auth::AuthLayer;
