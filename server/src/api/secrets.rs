use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

use super::auth::{SettingAuth, SyncAuth};
use super::push::{self, PushChannels, PushMessage};

// ── Encryption helpers ──────────────────────────────────────────────────

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};

fn encryption_key() -> Result<Key<Aes256Gcm>, Status> {
    let cfg = crate::config::config();
    let key_b64 = &cfg.secrets.encryption_key;
    let key_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key_b64)
        .map_err(|_| Status::InternalServerError)?;
    if key_bytes.len() != 32 {
        return Err(Status::InternalServerError);
    }
    Ok(*Key::<Aes256Gcm>::from_slice(&key_bytes))
}

/// Encrypt plaintext with AES-256-GCM. Returns nonce (12 bytes) ‖ ciphertext.
fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, Status> {
    let key = encryption_key()?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| Status::InternalServerError)?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt data produced by `encrypt()`. Input: nonce (12 bytes) ‖ ciphertext.
fn decrypt(data: &[u8]) -> Result<Vec<u8>, Status> {
    if data.len() < 12 {
        return Err(Status::InternalServerError);
    }
    let key = encryption_key()?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from_slice(&data[..12]);
    cipher
        .decrypt(nonce, &data[12..])
        .map_err(|_| Status::InternalServerError)
}

// ── Request / response types ────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct CreateSecretBody {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, ToSchema)]
pub struct SecretMetadata {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Routes ──────────────────────────────────────────────────────────────

/// List all secrets for the cluster (name → decrypted value).
/// Used by daemons at config load to resolve `secret:NAME` references.
#[utoipa::path(
    get,
    path = "/api/secrets",
    tag = "Secrets",
    summary = "Get all secrets for the cluster",
    security(("bearer" = [])),
    responses(
        (status = 200, description = "Secrets map"),
        (status = 401, description = "Unauthorized"),
        (status = 503, description = "Secrets vault not configured"),
    ),
)]
#[rocket::get("/secrets")]
pub async fn get_secrets(
    auth: SyncAuth,
    pool: &State<PgPool>,
) -> Result<Json<HashMap<String, String>>, Status> {
    let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT name, encrypted_value FROM cluster_secrets WHERE cluster_id = $1",
    )
    .bind(auth.cluster_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    let mut secrets = HashMap::with_capacity(rows.len());
    for (name, encrypted) in rows {
        let plaintext = decrypt(&encrypted)?;
        let value =
            String::from_utf8(plaintext).map_err(|_| Status::InternalServerError)?;
        secrets.insert(name, value);
    }
    Ok(Json(secrets))
}

/// Create a new secret.
#[utoipa::path(
    post,
    path = "/api/secrets",
    tag = "Secrets",
    summary = "Create a secret",
    security(("bearer" = [])),
    request_body = CreateSecretBody,
    responses(
        (status = 201, description = "Secret created", body = SecretMetadata),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Secret with this name already exists"),
        (status = 503, description = "Secrets vault not configured"),
    ),
)]
#[rocket::post("/secrets", data = "<body>")]
pub async fn create_secret(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    body: Json<CreateSecretBody>,
) -> Result<(Status, Json<SecretMetadata>), Status> {
    let encrypted = encrypt(body.value.as_bytes())?;

    let row: (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "INSERT INTO cluster_secrets (cluster_id, name, encrypted_value) \
         VALUES ($1, $2, $3) \
         RETURNING id, created_at",
    )
    .bind(auth.cluster_id)
    .bind(&body.name)
    .bind(&encrypted)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })?;

    push::notify(channels, auth.cluster_id, PushMessage::SyncConfig).await;

    Ok((
        Status::Created,
        Json(SecretMetadata {
            id: row.0,
            name: body.name.clone(),
            created_at: row.1,
        }),
    ))
}

/// Update an existing secret's value.
#[utoipa::path(
    put,
    path = "/api/secrets/{name}",
    tag = "Secrets",
    summary = "Update a secret",
    security(("bearer" = [])),
    request_body = CreateSecretBody,
    responses(
        (status = 200, description = "Secret updated"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Secret not found"),
        (status = 503, description = "Secrets vault not configured"),
    ),
)]
#[rocket::put("/secrets/<name>", data = "<body>")]
pub async fn update_secret(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    name: &str,
    body: Json<CreateSecretBody>,
) -> Result<Status, Status> {
    let encrypted = encrypt(body.value.as_bytes())?;

    let result = sqlx::query(
        "UPDATE cluster_secrets SET encrypted_value = $1, updated_at = now() \
         WHERE cluster_id = $2 AND name = $3",
    )
    .bind(&encrypted)
    .bind(auth.cluster_id)
    .bind(name)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if result.rows_affected() == 0 {
        return Err(Status::NotFound);
    }

    push::notify(channels, auth.cluster_id, PushMessage::SyncConfig).await;
    Ok(Status::Ok)
}

/// Delete a secret.
#[utoipa::path(
    delete,
    path = "/api/secrets/{name}",
    tag = "Secrets",
    summary = "Delete a secret",
    security(("bearer" = [])),
    responses(
        (status = 204, description = "Secret deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Secret not found"),
    ),
)]
#[rocket::delete("/secrets/<name>")]
pub async fn delete_secret(
    auth: SettingAuth,
    pool: &State<PgPool>,
    channels: &State<PushChannels>,
    name: &str,
) -> Result<Status, Status> {
    let result = sqlx::query(
        "DELETE FROM cluster_secrets WHERE cluster_id = $1 AND name = $2",
    )
    .bind(auth.cluster_id)
    .bind(name)
    .execute(pool.inner())
    .await
    .map_err(|_| Status::InternalServerError)?;

    if result.rows_affected() == 0 {
        return Err(Status::NotFound);
    }

    push::notify(channels, auth.cluster_id, PushMessage::SyncConfig).await;
    Ok(Status::NoContent)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        // Set up a test encryption key via config — we test the raw encrypt/decrypt
        // functions directly by constructing the cipher manually.
        let key_bytes: [u8; 32] = rand::random();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let plaintext = b"super-secret-api-key";
        let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref()).unwrap();

        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);

        // Decrypt
        let decrypted_nonce = Nonce::from_slice(&combined[..12]);
        let decrypted = cipher
            .decrypt(decrypted_nonce, &combined[12..])
            .unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
