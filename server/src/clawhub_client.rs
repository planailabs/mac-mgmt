//! Minimal read-only HTTP client for the ClawHub skill registry.
//!
//! ClawHub (<https://clawhub.ai>) is a public registry for OpenClaw agent
//! skills. This client supports searching, fetching skill details, and
//! downloading skill archives — all public endpoints, no authentication
//! required.

use serde::Deserialize;

pub struct ClawHubClient {
    http: reqwest::Client,
    base_url: String,
}

// ── Response types (only fields we need) ───────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub slug: String,
    pub display_name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    pub slug: String,
    pub display_name: String,
    #[serde(default)]
    pub summary: String,
    pub latest_version: Option<LatestVersion>,
}

#[derive(Debug, Deserialize)]
pub struct LatestVersion {
    pub version: String,
}

// ── Client implementation ──────────────────────────────────────────────

impl ClawHubClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build ClawHub HTTP client"),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Search for skills.
    ///
    /// Calls `GET /api/v1/search?q={query}&limit={limit}`.
    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchResult>, String> {
        let resp = self
            .http
            .get(format!("{}/api/v1/search", self.base_url))
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| format!("clawhub search failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("clawhub returned {}", resp.status()));
        }

        let body: SearchResponse = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse clawhub search response: {e}"))?;

        Ok(body.results)
    }

    /// Get skill details.
    ///
    /// Calls `GET /api/v1/skills/{slug}`.
    pub async fn get_skill(&self, slug: &str) -> Result<SkillDetail, String> {
        let resp = self
            .http
            .get(format!("{}/api/v1/skills/{slug}", self.base_url))
            .send()
            .await
            .map_err(|e| format!("clawhub get_skill failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("clawhub returned {}", resp.status()));
        }

        resp.json()
            .await
            .map_err(|e| format!("failed to parse clawhub skill response: {e}"))
    }

    /// Download a skill archive as raw bytes (zip).
    ///
    /// Calls `GET /api/v1/download?slug={slug}[&version={version}]`.
    pub async fn download(&self, slug: &str, version: Option<&str>) -> Result<Vec<u8>, String> {
        let mut req = self
            .http
            .get(format!("{}/api/v1/download", self.base_url))
            .query(&[("slug", slug)]);

        if let Some(v) = version {
            req = req.query(&[("version", v)]);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("clawhub download failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("clawhub download returned {}", resp.status()));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("failed to read clawhub download: {e}"))
    }
}
