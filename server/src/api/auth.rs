use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub struct AuthenticatedCustomer {
    pub customer_id: Uuid,
    pub token_kind: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedCustomer {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let pool = match req.rocket().state::<PgPool>() {
            Some(p) => p,
            None => return Outcome::Error((Status::InternalServerError, "no database pool")),
        };

        let header = match req.headers().get_one("Authorization") {
            Some(h) => h,
            None => return Outcome::Error((Status::Unauthorized, "missing Authorization header")),
        };

        let token = match header.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return Outcome::Error((Status::Unauthorized, "invalid Authorization format")),
        };

        let hash = hex::encode(Sha256::digest(token.as_bytes()));

        let result = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT customer_id, kind FROM tokens WHERE token_hash = $1 AND NOT revoked",
        )
        .bind(&hash)
        .fetch_optional(pool)
        .await;

        match result {
            Ok(Some((customer_id, kind))) => Outcome::Success(AuthenticatedCustomer {
                customer_id,
                token_kind: kind,
            }),
            Ok(None) => Outcome::Error((Status::Unauthorized, "invalid or revoked token")),
            Err(_) => Outcome::Error((Status::InternalServerError, "database error")),
        }
    }
}

/// Guard that only allows sync tokens.
pub struct SyncAuth {
    pub customer_id: Uuid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SyncAuth {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match AuthenticatedCustomer::from_request(req).await {
            Outcome::Success(auth) if auth.token_kind == "sync" => {
                Outcome::Success(SyncAuth {
                    customer_id: auth.customer_id,
                })
            }
            Outcome::Success(_) => Outcome::Error((Status::Forbidden, "sync token required")),
            Outcome::Error(e) => Outcome::Error(e),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}

/// Guard that only allows setting tokens.
pub struct SettingAuth {
    pub customer_id: Uuid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SettingAuth {
    type Error = &'static str;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match AuthenticatedCustomer::from_request(req).await {
            Outcome::Success(auth) if auth.token_kind == "setting" => {
                Outcome::Success(SettingAuth {
                    customer_id: auth.customer_id,
                })
            }
            Outcome::Success(_) => Outcome::Error((Status::Forbidden, "setting token required")),
            Outcome::Error(e) => Outcome::Error(e),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}
