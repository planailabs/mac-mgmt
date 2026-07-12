//! Pluggable Bearer-token authentication.
//!
//! The framework is storage-agnostic: a consumer implements [`Authenticator`]
//! to turn a raw bearer token into a [`Principal`] (e.g. by hashing it and
//! looking it up in a tokens table).

use async_trait::async_trait;

use crate::error::ApiError;
use crate::principal::Principal;

#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Resolve a raw bearer token (the part after `Bearer `) to a principal.
    /// Return [`ApiError::Unauthorized`] for unknown/expired tokens.
    async fn authenticate(&self, bearer: &str) -> Result<Principal, ApiError>;
}

/// Extract the bearer token from an `Authorization` header value.
pub fn bearer_from_header(headers: &http::HeaderMap) -> Result<&str, ApiError> {
    headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|t| !t.is_empty())
        .ok_or_else(|| ApiError::unauthorized("missing Authorization: Bearer <token> header"))
}
