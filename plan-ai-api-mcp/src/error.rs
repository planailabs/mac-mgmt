//! The framework error type, mappable to both HTTP responses and MCP errors.

use axum::Json;
use axum::response::{IntoResponse, Response};
use http::StatusCode;

/// An endpoint failure. Handlers return `Result<Output, ApiError>`; the
/// framework renders it to an HTTP status + JSON body or an MCP tool error.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// A short machine-readable error code for JSON/MCP bodies.
    pub fn code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Unauthorized(_) => "unauthorized",
            ApiError::Forbidden(_) => "forbidden",
            ApiError::NotFound(_) => "not_found",
            ApiError::Conflict(_) => "conflict",
            ApiError::Internal(_) => "internal",
        }
    }

    /// Render to an rmcp protocol error (used for auth failures at the MCP boundary).
    pub fn into_mcp(self) -> rmcp::model::ErrorData {
        use rmcp::model::{ErrorCode, ErrorData};
        let code = match self {
            ApiError::BadRequest(_) => ErrorCode::INVALID_PARAMS,
            ApiError::NotFound(_) => ErrorCode::INVALID_PARAMS,
            ApiError::Unauthorized(_) | ApiError::Forbidden(_) => ErrorCode::INVALID_REQUEST,
            _ => ErrorCode::INTERNAL_ERROR,
        };
        ErrorData::new(code, self.to_string(), None)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({
            "error": self.code(),
            "message": self.to_string(),
        }));
        (self.status(), body).into_response()
    }
}
