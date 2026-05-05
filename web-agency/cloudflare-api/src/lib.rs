//! Typed Cloudflare API v4 client.
//!
//! Covers zones, DNS records, DNSSEC, SSL settings, Pages, and Registrar
//! endpoints needed by the web-agency-server.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub mod types;

pub use types::*;

const BASE_URL: &str = "https://api.cloudflare.com/client/v4";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Cloudflare API error: {0:?}")]
    Api(Vec<ApiError>),
    #[error("Unexpected response: {0}")]
    Unexpected(String),
}

/// Cloudflare API error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: i64,
    pub message: String,
}

// Envelope is parsed dynamically via serde_json::Value to avoid
// Default trait bound issues with generic result types.

pub struct Client {
    http: reqwest::Client,
    base_url: String,
}

impl Client {
    /// Create a new Cloudflare API client with the given bearer token.
    pub fn new(api_token: &str) -> Self {
        Self::with_base_url(BASE_URL, api_token)
    }

    /// Create a client with a custom base URL (for testing with mock servers).
    pub fn with_base_url(base_url: &str, api_token: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_token}")).expect("invalid token"),
        );
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
        self.parse_envelope(resp).await
    }

    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T, Error> {
        let resp = self.http.post(format!("{}{path}", self.base_url)).json(body).send().await?;
        self.parse_envelope(resp).await
    }

    async fn patch<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T, Error> {
        let resp = self.http.patch(format!("{}{path}", self.base_url)).json(body).send().await?;
        self.parse_envelope(resp).await
    }

    async fn delete_req(&self, path: &str) -> Result<(), Error> {
        let resp = self.http.delete(format!("{}{path}", self.base_url)).send().await?;
        let body: serde_json::Value = resp.json().await?;
        let success = body.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if !success {
            let errors: Vec<ApiError> = body.get("errors")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            return Err(Error::Api(errors));
        }
        Ok(())
    }

    async fn parse_envelope<T: DeserializeOwned>(&self, resp: reqwest::Response) -> Result<T, Error> {
        let body: serde_json::Value = resp.json().await?;
        let success = body.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if !success {
            let errors: Vec<ApiError> = body.get("errors")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            return Err(Error::Api(errors));
        }
        let result = body.get("result")
            .ok_or_else(|| Error::Unexpected("missing result".into()))?;
        serde_json::from_value(result.clone())
            .map_err(|e| Error::Unexpected(format!("failed to parse result: {e}")))
    }

    // ── Accounts ──────────────────────────────────────────────────────

    /// List accounts accessible by this token.
    pub async fn list_accounts(&self) -> Result<Vec<Account>, Error> {
        self.get("/accounts").await
    }

    /// Resolve the account ID: if provided, use it; if empty, fetch the first
    /// account from the API (works for user-level API tokens).
    pub async fn resolve_account_id(&self, configured: &str) -> Result<String, Error> {
        if !configured.is_empty() {
            return Ok(configured.to_string());
        }
        let accounts = self.list_accounts().await?;
        accounts
            .first()
            .map(|a| a.id.clone())
            .ok_or_else(|| Error::Unexpected("no accounts accessible by this token".into()))
    }

    // ── Zones ─────────────────────────────────────────────────────────

    /// List zones, optionally filtered by name.
    pub async fn list_zones(&self, name: Option<&str>) -> Result<Vec<Zone>, Error> {
        let path = match name {
            Some(n) => format!("/zones?name={n}&per_page=50"),
            None => "/zones?per_page=50".to_string(),
        };
        self.get(&path).await
    }

    /// Create a new zone.
    pub async fn create_zone(&self, name: &str, account_id: &str) -> Result<Zone, Error> {
        #[derive(Serialize)]
        struct Body<'a> { name: &'a str, account: AccountRef<'a> }
        #[derive(Serialize)]
        struct AccountRef<'a> { id: &'a str }
        self.post("/zones", &Body { name, account: AccountRef { id: account_id } }).await
    }

    /// Get zone details.
    pub async fn get_zone(&self, zone_id: &str) -> Result<Zone, Error> {
        self.get(&format!("/zones/{zone_id}")).await
    }

    /// Delete a zone.
    pub async fn delete_zone(&self, zone_id: &str) -> Result<(), Error> {
        self.delete_req(&format!("/zones/{zone_id}")).await
    }

    // ── DNS Records ───────────────────────────────────────────────────

    /// List DNS records for a zone.
    pub async fn list_dns_records(&self, zone_id: &str) -> Result<Vec<DnsRecord>, Error> {
        self.get(&format!("/zones/{zone_id}/dns_records?per_page=500")).await
    }

    /// Create a DNS record.
    pub async fn create_dns_record(&self, zone_id: &str, record: &CreateDnsRecord) -> Result<DnsRecord, Error> {
        self.post(&format!("/zones/{zone_id}/dns_records"), record).await
    }

    /// Update a DNS record (partial).
    pub async fn update_dns_record(&self, zone_id: &str, record_id: &str, record: &UpdateDnsRecord) -> Result<DnsRecord, Error> {
        self.patch(&format!("/zones/{zone_id}/dns_records/{record_id}"), record).await
    }

    /// Delete a DNS record.
    pub async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<(), Error> {
        self.delete_req(&format!("/zones/{zone_id}/dns_records/{record_id}")).await
    }

    // ── DNSSEC ────────────────────────────────────────────────────────

    /// Get DNSSEC status for a zone.
    pub async fn get_dnssec(&self, zone_id: &str) -> Result<DnssecStatus, Error> {
        self.get(&format!("/zones/{zone_id}/dnssec")).await
    }

    /// Enable or disable DNSSEC.
    pub async fn set_dnssec(&self, zone_id: &str, status: &str) -> Result<DnssecStatus, Error> {
        #[derive(Serialize)]
        struct Body<'a> { status: &'a str }
        self.patch(&format!("/zones/{zone_id}/dnssec"), &Body { status }).await
    }

    // ── SSL/TLS ───────────────────────────────────────────────────────

    /// Get SSL automatic mode setting.
    pub async fn get_ssl_mode(&self, zone_id: &str) -> Result<SslMode, Error> {
        self.get(&format!("/zones/{zone_id}/settings/ssl_automatic_mode")).await
    }

    /// Set SSL automatic mode (auto or custom).
    pub async fn set_ssl_mode(&self, zone_id: &str, value: &str) -> Result<SslMode, Error> {
        #[derive(Serialize)]
        struct Body<'a> { value: &'a str }
        self.patch(&format!("/zones/{zone_id}/settings/ssl_automatic_mode"), &Body { value }).await
    }

    // ── Registrar ─────────────────────────────────────────────────────

    /// Check domain availability and pricing.
    pub async fn check_domains(&self, account_id: &str, domains: &[String]) -> Result<Vec<DomainCheck>, Error> {
        #[derive(Serialize)]
        struct Body<'a> { domains: &'a [String] }
        self.post(
            &format!("/accounts/{account_id}/registrar/domain-check"),
            &Body { domains },
        ).await
    }

    /// List registered domains.
    pub async fn list_registrations(&self, account_id: &str) -> Result<Vec<Registration>, Error> {
        self.get(&format!("/accounts/{account_id}/registrar/registrations?per_page=50")).await
    }

    /// Get registration details.
    pub async fn get_registration(&self, account_id: &str, domain: &str) -> Result<Registration, Error> {
        self.get(&format!("/accounts/{account_id}/registrar/registrations/{domain}")).await
    }

    // ── Pages ─────────────────────────────────────────────────────────

    /// List Cloudflare Pages projects.
    pub async fn list_pages_projects(&self, account_id: &str) -> Result<Vec<PagesProject>, Error> {
        self.get(&format!("/accounts/{account_id}/pages/projects")).await
    }

    /// Create a Pages project.
    pub async fn create_pages_project(&self, account_id: &str, name: &str, production_branch: &str) -> Result<PagesProject, Error> {
        #[derive(Serialize)]
        struct Body<'a> { name: &'a str, production_branch: &'a str }
        self.post(&format!("/accounts/{account_id}/pages/projects"), &Body { name, production_branch }).await
    }

    /// Get a Pages project.
    pub async fn get_pages_project(&self, account_id: &str, project_name: &str) -> Result<PagesProject, Error> {
        self.get(&format!("/accounts/{account_id}/pages/projects/{project_name}")).await
    }

    /// Update a Pages project (source, build config, production branch).
    pub async fn update_pages_project(&self, account_id: &str, project_name: &str, update: &UpdatePagesProject) -> Result<PagesProject, Error> {
        self.patch(&format!("/accounts/{account_id}/pages/projects/{project_name}"), update).await
    }

    /// Delete a Pages project.
    pub async fn delete_pages_project(&self, account_id: &str, project_name: &str) -> Result<(), Error> {
        self.delete_req(&format!("/accounts/{account_id}/pages/projects/{project_name}")).await
    }

    /// List custom domains for a Pages project.
    pub async fn list_pages_custom_domains(&self, account_id: &str, project_name: &str) -> Result<Vec<PagesCustomDomain>, Error> {
        self.get(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains")).await
    }

    /// Get a specific custom domain's status.
    pub async fn get_pages_custom_domain(&self, account_id: &str, project_name: &str, domain: &str) -> Result<PagesCustomDomain, Error> {
        self.get(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains/{domain}")).await
    }

    /// Add a custom domain to a Pages project.
    pub async fn add_pages_custom_domain(&self, account_id: &str, project_name: &str, domain: &str) -> Result<PagesCustomDomain, Error> {
        #[derive(Serialize)]
        struct Body<'a> { name: &'a str }
        self.post(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains"), &Body { name: domain }).await
    }

    /// Remove a custom domain from a Pages project.
    pub async fn remove_pages_custom_domain(&self, account_id: &str, project_name: &str, domain: &str) -> Result<(), Error> {
        self.delete_req(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains/{domain}")).await
    }
}
