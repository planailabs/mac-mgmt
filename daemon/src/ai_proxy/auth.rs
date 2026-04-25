use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};

use super::{AiProxyState, multihash_key};

/// Authenticated API key — extracted from `Authorization: Bearer <key>`.
/// The incoming key is hashed with SHA2-256 multihash and compared against
/// the stored `key_hash` in the config. Budget is checked during extraction;
/// requests over budget are rejected.
pub struct AuthedKey {
    pub key_hash: String,
    pub key_name: String,
    pub token_budget: i64,
    pub budget_window: std::time::Duration,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthedKey {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let state = match request.rocket().state::<std::sync::Arc<AiProxyState>>() {
            Some(s) => s,
            None => return Outcome::Error((Status::InternalServerError, "missing state")),
        };

        // Extract Bearer token
        let auth_header = match request.headers().get_one("Authorization") {
            Some(h) => h,
            None => {
                return Outcome::Error((
                    Status::Unauthorized,
                    "missing Authorization header",
                ))
            }
        };

        let token = match auth_header.strip_prefix("Bearer ") {
            Some(t) => t.trim(),
            None => {
                return Outcome::Error((
                    Status::Unauthorized,
                    "Authorization must use Bearer scheme",
                ))
            }
        };

        if token.is_empty() {
            return Outcome::Error((Status::Unauthorized, "empty bearer token"));
        }

        // Hash the incoming key and compare against stored multihashes
        let hash = multihash_key(token);

        // Look up key by multihash
        let keys = state.keys.read().await;
        let entry = match keys.iter().find(|k| k.key_hash == hash) {
            Some(e) => e,
            None => return Outcome::Error((Status::Unauthorized, "invalid API key")),
        };

        if !entry.enabled {
            return Outcome::Error((Status::Forbidden, "API key is disabled"));
        }

        // Check sliding window budget
        let remaining = state
            .usage_tracker
            .remaining_budget(&entry.key_hash, entry.token_budget, entry.budget_window);

        if remaining <= 0 {
            return Outcome::Error((Status::TooManyRequests, "token budget exceeded"));
        }

        Outcome::Success(AuthedKey {
            key_hash: entry.key_hash.clone(),
            key_name: entry.name.clone(),
            token_budget: entry.token_budget,
            budget_window: entry.budget_window,
        })
    }
}
