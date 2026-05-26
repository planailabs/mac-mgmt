//! Compatibility wrapper for the progenitor-generated Spaceship API client.

use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error ({status}): {detail}")]
    Api { status: u16, detail: String },
    #[error("Async operation failed: {0}")]
    AsyncFailed(String),
    #[error("Async operation timed out")]
    AsyncTimeout,
}

// ── Simple types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AsyncOperation {
    pub status: String,
    #[serde(rename = "type")]
    pub op_type: Option<String>,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContactDetails {
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub address1: String,
    pub city: String,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<String>,
    #[serde(rename = "stateProvince", skip_serializing_if = "Option::is_none")]
    pub state_province: Option<String>,
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContactSaveResponse {
    #[serde(rename = "contactId")]
    pub contact_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomainAvailability {
    pub domain: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "premiumPricing", default)]
    pub premium_pricing: Vec<PremiumPrice>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PremiumPrice {
    pub operation: String,
    pub price: f64,
    pub currency: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomainListResponse {
    pub items: Vec<DomainInfo>,
    #[serde(rename = "totalCount")]
    pub total_count: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomainInfo {
    pub name: String,
    #[serde(rename = "autoRenew")]
    pub auto_renew: Option<bool>,
    #[serde(rename = "registrationDate")]
    pub registration_date: Option<String>,
    #[serde(rename = "expirationDate")]
    pub expiration_date: Option<String>,
    #[serde(rename = "lifecycleStatus")]
    pub lifecycle_status: Option<String>,
    #[serde(default)]
    pub nameservers: Option<NameserverConfig>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NameserverConfig {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
}

// ── Client ────────────────────────────────────────────────────────────

pub struct SimpleClient {
    http: reqwest::Client,
    base_url: String,
}

impl SimpleClient {
    pub fn new(api_key: &str, api_secret: &str) -> Self {
        Self::with_base_url("https://spaceship.dev/api", api_key, api_secret)
    }

    pub fn with_base_url(base_url: &str, api_key: &str, api_secret: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("X-Api-Key", HeaderValue::from_str(api_key).unwrap());
        headers.insert("X-Api-Secret", HeaderValue::from_str(api_secret).unwrap());
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let resp = self
            .http
            .get(format!("{}{path}", self.base_url))
            .send()
            .await?;
        self.check(resp).await
    }

    async fn put_json<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let resp = self
            .http
            .put(format!("{}{path}", self.base_url))
            .json(body)
            .send()
            .await?;
        self.check(resp).await
    }

    async fn check<T: DeserializeOwned>(&self, resp: reqwest::Response) -> Result<T, Error> {
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api {
                status,
                detail: text,
            });
        }
        Ok(resp.json().await?)
    }

    // ── Async operations ──────────────────────────────────────────

    pub async fn get_async_operation(&self, op_id: &str) -> Result<AsyncOperation, Error> {
        self.get(&format!("/v1/async-operations/{op_id}")).await
    }

    pub async fn poll_until_complete(
        &self,
        op_id: &str,
        timeout: Duration,
    ) -> Result<AsyncOperation, Error> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let op = self.get_async_operation(op_id).await?;
            match op.status.as_str() {
                "success" => return Ok(op),
                "failed" => {
                    return Err(Error::AsyncFailed(
                        serde_json::to_string(&op.details).unwrap_or_default(),
                    ));
                }
                _ => {
                    if tokio::time::Instant::now() > deadline {
                        return Err(Error::AsyncTimeout);
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    // ── Contacts ──────────────────────────────────────────────────

    pub async fn save_contact(
        &self,
        contact: &ContactDetails,
    ) -> Result<ContactSaveResponse, Error> {
        self.put_json("/v1/contacts", contact).await
    }

    pub async fn get_contact(&self, contact_id: &str) -> Result<ContactDetails, Error> {
        self.get(&format!("/v1/contacts/{contact_id}")).await
    }

    // ── Domain availability ───────────────────────────────────────

    pub async fn check_availability(&self, domain: &str) -> Result<DomainAvailability, Error> {
        self.get(&format!("/v1/domains/{domain}/available")).await
    }

    // ── Domain management ─────────────────────────────────────────

    pub async fn list_domains(&self, skip: u32, take: u32) -> Result<DomainListResponse, Error> {
        let take = take.min(100);
        self.get(&format!("/v1/domains?skip={skip}&take={take}"))
            .await
    }

    pub async fn list_all_domains(&self) -> Result<Vec<DomainInfo>, Error> {
        let mut all = Vec::new();
        let mut skip = 0u32;
        loop {
            let resp = self.list_domains(skip, 100).await?;
            let count = resp.items.len() as u32;
            all.extend(resp.items);
            if count < 100 {
                break;
            }
            skip += count;
        }
        Ok(all)
    }

    pub async fn get_domain_info(&self, domain: &str) -> Result<DomainInfo, Error> {
        self.get(&format!("/v1/domains/{domain}")).await
    }

    // ── Domain settings ───────────────────────────────────────────

    pub async fn set_nameservers(
        &self,
        domain: &str,
        config: &NameserverConfig,
    ) -> Result<(), Error> {
        let resp = self
            .http
            .put(format!("{}/v1/domains/{domain}/nameservers", self.base_url))
            .json(config)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api {
                status,
                detail: text,
            });
        }
        Ok(())
    }

    pub async fn set_autorenew(&self, domain: &str, enabled: bool) -> Result<(), Error> {
        #[derive(serde::Serialize)]
        struct Body {
            #[serde(rename = "isEnabled")]
            is_enabled: bool,
        }
        let resp = self
            .http
            .put(format!("{}/v1/domains/{domain}/autorenew", self.base_url))
            .json(&Body {
                is_enabled: enabled,
            })
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api {
                status,
                detail: text,
            });
        }
        Ok(())
    }
}
