//! Typed Spaceship.com API client.
//!
//! Covers domains, contacts, DNS records, availability, and async operations.
//! Auth: X-Api-Key + X-Api-Secret headers.

use reqwest::header::{HeaderMap, HeaderValue};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

pub mod types;

pub use types::*;

const BASE_URL: &str = "https://spaceship.dev/api";

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

pub struct Client {
    http: reqwest::Client,
    base_url: String,
}

impl Client {
    pub fn new(api_key: &str, api_secret: &str) -> Self {
        Self::with_base_url(BASE_URL, api_key, api_secret)
    }

    pub fn with_base_url(base_url: &str, api_key: &str, api_secret: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("X-Api-Key", HeaderValue::from_str(api_key).expect("invalid api key"));
        headers.insert("X-Api-Secret", HeaderValue::from_str(api_secret).expect("invalid api secret"));
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let resp = self.http.get(format!("{}{path}", self.base_url)).send().await?;
        self.check_response(resp).await
    }

    async fn post_json<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T, Error> {
        let resp = self.http.post(format!("{}{path}", self.base_url)).json(body).send().await?;
        self.check_response(resp).await
    }

    async fn put_json<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T, Error> {
        let resp = self.http.put(format!("{}{path}", self.base_url)).json(body).send().await?;
        self.check_response(resp).await
    }

    async fn delete_json<B: Serialize>(&self, path: &str, body: &B) -> Result<(), Error> {
        let resp = self.http.delete(format!("{}{path}", self.base_url)).json(body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, detail: text });
        }
        Ok(())
    }

    /// POST that returns 202 Accepted with an async operation ID in the response header.
    async fn post_async<B: Serialize>(&self, path: &str, body: &B) -> Result<String, Error> {
        let resp = self.http.post(format!("{}{path}", self.base_url)).json(body).send().await?;
        let status = resp.status().as_u16();
        if status != 202 && !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, detail: text });
        }
        let op_id = resp
            .headers()
            .get("spaceship-async-operationid")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_default();
        Ok(op_id)
    }

    async fn check_response<T: DeserializeOwned>(&self, resp: reqwest::Response) -> Result<T, Error> {
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, detail: text });
        }
        Ok(resp.json().await?)
    }

    // ── Async Operations ──────────────────────────────────────────────

    /// Get async operation status.
    pub async fn get_async_operation(&self, operation_id: &str) -> Result<AsyncOperation, Error> {
        self.get(&format!("/v1/async-operations/{operation_id}")).await
    }

    /// Poll an async operation until it reaches a terminal state.
    pub async fn poll_until_complete(&self, operation_id: &str, timeout: Duration) -> Result<AsyncOperation, Error> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let op = self.get_async_operation(operation_id).await?;
            match op.status.as_str() {
                "success" => return Ok(op),
                "failed" => return Err(Error::AsyncFailed(
                    serde_json::to_string(&op.details).unwrap_or_default(),
                )),
                _ => {
                    if tokio::time::Instant::now() > deadline {
                        return Err(Error::AsyncTimeout);
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    // ── Contacts ──────────────────────────────────────────────────────

    /// Save (create/update) a contact. Returns the contact ID.
    pub async fn save_contact(&self, contact: &ContactDetails) -> Result<ContactSaveResponse, Error> {
        self.put_json("/v1/contacts", contact).await
    }

    /// Read contact details.
    pub async fn get_contact(&self, contact_id: &str) -> Result<ContactDetails, Error> {
        self.get(&format!("/v1/contacts/{contact_id}")).await
    }

    // ── Domain Availability ───────────────────────────────────────────

    /// Check single domain availability.
    pub async fn check_availability(&self, domain: &str) -> Result<DomainAvailability, Error> {
        self.get(&format!("/v1/domains/{domain}/available")).await
    }

    /// Batch check availability (max 20 domains).
    pub async fn check_availability_batch(&self, domains: &[String]) -> Result<Vec<DomainAvailability>, Error> {
        #[derive(Serialize)]
        struct Body<'a> { domains: &'a [String] }
        self.post_json("/v1/domains/available", &Body { domains }).await
    }

    // ── Domain Management ─────────────────────────────────────────────

    /// List domains (paginated).
    pub async fn list_domains(&self, skip: u32, take: u32) -> Result<DomainListResponse, Error> {
        self.get(&format!("/v1/domains?skip={skip}&take={take}")).await
    }

    /// Get domain info.
    pub async fn get_domain_info(&self, domain: &str) -> Result<DomainInfo, Error> {
        self.get(&format!("/v1/domains/{domain}")).await
    }

    /// Register a domain (async). Returns the operation ID.
    pub async fn register_domain(&self, domain: &str, req: &RegisterRequest) -> Result<String, Error> {
        self.post_async(&format!("/v1/domains/{domain}"), req).await
    }

    /// Renew a domain (async). Returns the operation ID.
    pub async fn renew_domain(&self, domain: &str, years: u32, current_expiration: &str) -> Result<String, Error> {
        #[derive(Serialize)]
        struct Body<'a> {
            years: u32,
            #[serde(rename = "currentExpirationDate")]
            current_expiration_date: &'a str,
        }
        self.post_async(
            &format!("/v1/domains/{domain}/renew"),
            &Body { years, current_expiration_date: current_expiration },
        ).await
    }

    /// Delete a domain.
    pub async fn delete_domain(&self, domain: &str) -> Result<(), Error> {
        let resp = self.http.delete(format!("{}/v1/domains/{domain}", self.base_url)).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, detail: text });
        }
        Ok(())
    }

    // ── Domain Settings ───────────────────────────────────────────────

    /// Set nameservers for a domain.
    pub async fn set_nameservers(&self, domain: &str, config: &NameserverConfig) -> Result<(), Error> {
        let resp = self.http.put(format!("{}/v1/domains/{domain}/nameservers", self.base_url))
            .json(config).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, detail: text });
        }
        Ok(())
    }

    /// Set auto-renew for a domain.
    pub async fn set_autorenew(&self, domain: &str, enabled: bool) -> Result<(), Error> {
        #[derive(Serialize)]
        struct Body { #[serde(rename = "isEnabled")] is_enabled: bool }
        let resp = self.http.put(format!("{}/v1/domains/{domain}/autorenew", self.base_url))
            .json(&Body { is_enabled: enabled }).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, detail: text });
        }
        Ok(())
    }

    // ── DNS Records ───────────────────────────────────────────────────

    /// List DNS records for a domain.
    pub async fn list_dns_records(&self, domain: &str, skip: u32, take: u32) -> Result<DnsRecordListResponse, Error> {
        self.get(&format!("/v1/dns/records/{domain}?skip={skip}&take={take}")).await
    }

    /// Save (create/update) DNS records.
    pub async fn save_dns_records(&self, domain: &str, records: &[DnsRecord], force: bool) -> Result<(), Error> {
        #[derive(Serialize)]
        struct Body<'a> { items: &'a [DnsRecord], force: bool }
        let resp = self.http.put(format!("{}/v1/dns/records/{domain}", self.base_url))
            .json(&Body { items: records, force }).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(Error::Api { status, detail: text });
        }
        Ok(())
    }

    /// Delete DNS records.
    pub async fn delete_dns_records(&self, domain: &str, records: &[DnsRecord]) -> Result<(), Error> {
        let records_vec: Vec<&DnsRecord> = records.iter().collect();
        self.delete_json(&format!("/v1/dns/records/{domain}"), &records_vec).await
    }

    // ── Domain Transfer ───────────────────────────────────────────────

    /// Get the EPP auth code for a domain.
    pub async fn get_auth_code(&self, domain: &str) -> Result<AuthCodeResponse, Error> {
        self.get(&format!("/v1/domains/{domain}/transfer/auth-code")).await
    }
}
