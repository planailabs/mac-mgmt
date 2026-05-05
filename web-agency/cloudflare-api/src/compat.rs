//! Compatibility wrapper providing a simple interface using the progenitor-generated types
//! for serialization but raw reqwest for HTTP calls.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::de::DeserializeOwned;

/// Custom domain on a CF Pages project. Uses permissive deserialization
/// since the CF API may omit fields for domains in certain states.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PagesCustomDomain {
    #[serde(default)] pub id: Option<String>,
    #[serde(default)] pub name: String,
    #[serde(default)] pub status: Option<String>,
    #[serde(default)] pub certificate_authority: Option<String>,
    #[serde(default)] pub created_on: Option<String>,
    #[serde(default)] pub domain_id: Option<String>,
    #[serde(default)] pub zone_tag: Option<String>,
    #[serde(default)] pub validation_data: Option<serde_json::Value>,
    #[serde(default)] pub verification_data: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0:?}")]
    Api(Vec<ApiError>),
    #[error("Unexpected: {0}")]
    Unexpected(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiError { pub code: i64, pub message: String }

pub struct SimpleClient {
    http: reqwest::Client,
    base_url: String,
}

// ── Simple response types ─────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Account { pub id: String, pub name: String }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Zone {
    pub id: String, pub name: String, pub status: String,
    pub name_servers: Option<Vec<String>>,
    pub account: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DnsRecord {
    pub id: String, #[serde(rename = "type")] pub record_type: String,
    pub name: String, pub content: Option<String>,
    pub proxied: Option<bool>, pub comment: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateDnsRecord {
    #[serde(rename = "type")] pub record_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")] pub ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub proxied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub priority: Option<u16>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomainCheck {
    pub name: String, #[serde(default)] pub registrable: bool,
    pub tier: Option<String>, pub pricing: Option<DomainPricing>, pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DomainPricing { pub currency: String, pub registration_cost: Option<String>, pub renewal_cost: Option<String> }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PagesProject {
    pub name: String, pub id: Option<String>, pub subdomain: Option<String>,
    pub production_branch: Option<String>,
    pub source: Option<serde_json::Value>, pub build_config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UpdatePagesProject {
    #[serde(skip_serializing_if = "Option::is_none")] pub production_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub source: Option<PagesSource>,
    #[serde(skip_serializing_if = "Option::is_none")] pub build_config: Option<PagesBuildConfig>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PagesSource {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")] pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub config: Option<PagesSourceConfig>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PagesSourceConfig {
    #[serde(skip_serializing_if = "Option::is_none")] pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub repo_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub production_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub pr_comments_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub production_deployments_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub preview_deployment_setting: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub preview_branch_includes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")] pub preview_branch_excludes: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PagesBuildConfig {
    #[serde(skip_serializing_if = "Option::is_none")] pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub destination_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub root_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub build_caching: Option<bool>,
}

// ── HTTP helpers ──────────────────────────────────────────────────────

impl SimpleClient {
    pub fn new(api_token: &str) -> Self {
        Self::with_base_url("https://api.cloudflare.com/client/v4", api_token)
    }

    pub fn with_base_url(base_url: &str, api_token: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {api_token}")).unwrap());
        let http = reqwest::Client::builder().default_headers(headers).build().unwrap();
        Self { http, base_url: base_url.trim_end_matches('/').to_string() }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let resp = self.http.get(format!("{}{path}", self.base_url)).send().await?;
        self.parse(resp).await
    }

    async fn post<T: DeserializeOwned, B: serde::Serialize>(&self, path: &str, body: &B) -> Result<T, Error> {
        let resp = self.http.post(format!("{}{path}", self.base_url)).json(body).send().await?;
        self.parse(resp).await
    }

    async fn patch<T: DeserializeOwned, B: serde::Serialize>(&self, path: &str, body: &B) -> Result<T, Error> {
        let resp = self.http.patch(format!("{}{path}", self.base_url)).json(body).send().await?;
        self.parse(resp).await
    }

    async fn delete(&self, path: &str) -> Result<(), Error> {
        let resp = self.http.delete(format!("{}{path}", self.base_url)).send().await?;
        let body: serde_json::Value = resp.json().await?;
        if !body.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
            let errors: Vec<ApiError> = body.get("errors").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
            return Err(Error::Api(errors));
        }
        Ok(())
    }

    async fn parse<T: DeserializeOwned>(&self, resp: reqwest::Response) -> Result<T, Error> {
        let body: serde_json::Value = resp.json().await?;
        if !body.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
            let errors: Vec<ApiError> = body.get("errors").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
            return Err(Error::Api(errors));
        }
        let result = body.get("result").ok_or_else(|| Error::Unexpected("missing result".into()))?;
        serde_json::from_value(result.clone()).map_err(|e| Error::Unexpected(format!("parse: {e}")))
    }

    // ── API methods ───────────────────────────────────────────────

    pub async fn list_accounts(&self) -> Result<Vec<Account>, Error> { self.get("/accounts").await }

    pub async fn resolve_account_id(&self, configured: &str) -> Result<String, Error> {
        if !configured.is_empty() { return Ok(configured.to_string()); }
        let accts: Vec<Account> = self.list_accounts().await?;
        accts.first().map(|a| a.id.clone()).ok_or_else(|| Error::Unexpected("no accounts".into()))
    }

    pub async fn list_zones(&self, name: Option<&str>) -> Result<Vec<Zone>, Error> {
        let path = match name { Some(n) => format!("/zones?name={n}&per_page=50"), None => "/zones?per_page=50".into() };
        self.get(&path).await
    }

    pub async fn create_zone(&self, name: &str, account_id: &str) -> Result<Zone, Error> {
        self.post("/zones", &serde_json::json!({"name": name, "account": {"id": account_id}})).await
    }

    pub async fn get_zone(&self, zone_id: &str) -> Result<Zone, Error> { self.get(&format!("/zones/{zone_id}")).await }

    pub async fn list_dns_records(&self, zone_id: &str) -> Result<Vec<DnsRecord>, Error> {
        self.get(&format!("/zones/{zone_id}/dns_records?per_page=500")).await
    }

    pub async fn create_dns_record(&self, zone_id: &str, record: &CreateDnsRecord) -> Result<DnsRecord, Error> {
        self.post(&format!("/zones/{zone_id}/dns_records"), record).await
    }

    pub async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<(), Error> {
        self.delete(&format!("/zones/{zone_id}/dns_records/{record_id}")).await
    }

    pub async fn set_dnssec(&self, zone_id: &str, status: &str) -> Result<serde_json::Value, Error> {
        self.patch(&format!("/zones/{zone_id}/dnssec"), &serde_json::json!({"status": status})).await
    }

    pub async fn set_ssl_mode(&self, zone_id: &str, value: &str) -> Result<serde_json::Value, Error> {
        self.patch(&format!("/zones/{zone_id}/settings/ssl_automatic_mode"), &serde_json::json!({"value": value})).await
    }

    pub async fn check_domains(&self, account_id: &str, domains: &[String]) -> Result<Vec<DomainCheck>, Error> {
        #[derive(serde::Deserialize)] struct Res { domains: Option<Vec<DomainCheck>> }
        let r: Res = self.post(&format!("/accounts/{account_id}/registrar/domain-check"), &serde_json::json!({"domains": domains})).await?;
        Ok(r.domains.unwrap_or_default())
    }

    pub async fn get_pages_project(&self, account_id: &str, project_name: &str) -> Result<PagesProject, Error> {
        self.get(&format!("/accounts/{account_id}/pages/projects/{project_name}")).await
    }

    pub async fn create_pages_project(&self, account_id: &str, name: &str, branch: &str) -> Result<PagesProject, Error> {
        self.post(&format!("/accounts/{account_id}/pages/projects"), &serde_json::json!({"name": name, "production_branch": branch})).await
    }

    pub async fn update_pages_project(&self, account_id: &str, project_name: &str, update: &UpdatePagesProject) -> Result<PagesProject, Error> {
        self.patch(&format!("/accounts/{account_id}/pages/projects/{project_name}"), update).await
    }

    pub async fn list_pages_custom_domains(&self, account_id: &str, project_name: &str) -> Result<Vec<PagesCustomDomain>, Error> {
        let path = format!("/accounts/{account_id}/pages/projects/{project_name}/domains");
        let url = format!("{}{path}", self.base_url);
        tracing::debug!(url = %url, "fetching Pages custom domains");
        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        let body_text = resp.text().await?;
        tracing::debug!(
            http_status = %status,
            body_len = body_text.len(),
            body_preview = %&body_text[..body_text.len().min(500)],
            "Pages custom domains raw response"
        );
        let body: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| Error::Unexpected(format!("JSON parse error: {e}")))?;
        let success = body.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if !success {
            let errors: Vec<ApiError> = body.get("errors").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
            tracing::warn!(success = false, errors = ?errors, "CF API returned failure");
            return Err(Error::Api(errors));
        }
        let result = body.get("result");
        tracing::debug!(
            result_type = ?result.map(|r| match r { serde_json::Value::Array(a) => format!("array[{}]", a.len()), _ => format!("{}", r) }),
            "Pages domains result field"
        );
        let arr = result.and_then(|r| r.as_array()).cloned().unwrap_or_default();
        let mut domains = Vec::new();
        for (i, item) in arr.iter().enumerate() {
            tracing::debug!(index = i, raw = %item, "parsing Pages domain entry");
            match serde_json::from_value::<PagesCustomDomain>(item.clone()) {
                Ok(d) => {
                    tracing::debug!(index = i, name = %d.name, status = ?d.status, "parsed domain OK");
                    domains.push(d);
                }
                Err(e) => {
                    tracing::warn!(index = i, error = %e, raw = %item, "failed to parse Pages domain entry");
                }
            }
        }
        tracing::info!(
            project = %project_name,
            total_from_api = arr.len(),
            parsed_ok = domains.len(),
            names = ?domains.iter().map(|d| &d.name).collect::<Vec<_>>(),
            "list_pages_custom_domains result"
        );
        Ok(domains)
    }

    pub async fn get_pages_custom_domain(&self, account_id: &str, project_name: &str, domain: &str) -> Result<PagesCustomDomain, Error> {
        self.get(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains/{domain}")).await
    }

    pub async fn add_pages_custom_domain(&self, account_id: &str, project_name: &str, domain: &str) -> Result<PagesCustomDomain, Error> {
        self.post(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains"), &serde_json::json!({"name": domain})).await
    }

    pub async fn remove_pages_custom_domain(&self, account_id: &str, project_name: &str, domain: &str) -> Result<(), Error> {
        self.delete(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains/{domain}")).await
    }

    pub async fn retry_pages_custom_domain(&self, account_id: &str, project_name: &str, domain: &str) -> Result<PagesCustomDomain, Error> {
        self.patch(&format!("/accounts/{account_id}/pages/projects/{project_name}/domains/{domain}"), &serde_json::json!({})).await
    }
}
