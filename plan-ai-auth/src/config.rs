use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    pub cookie_secret: String,
    /// External base URL of the web UI (e.g. "https://mgmt.example.com").
    /// Used to derive OIDC callback URLs (`{external_url}/auth/{slug}/callback`).
    #[serde(default = "default_auth_external_url")]
    pub external_url: String,
    /// Optional Redis URL for session cache. If absent, PostgreSQL is used.
    pub redis_url: Option<String>,
    /// Emails that are automatically granted admin on first login.
    #[serde(default)]
    pub admin_emails: Vec<String>,
    /// OIDC providers. Each gets its own auth routes and access control.
    pub providers: Vec<OidcProviderConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcProviderConfig {
    /// URL slug used in auth routes: /auth/{slug}, /auth/{slug}/callback
    pub slug: String,
    /// Human-readable name shown on the login page.
    pub name: String,
    /// OIDC issuer URL for auto-discovery (e.g. "https://accounts.google.com").
    pub issuer: Option<String>,
    pub client_id: String,
    pub client_secret: String,
    /// Allow any authenticated user from this provider.
    #[serde(default)]
    pub allow_all: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub allowed_emails: Vec<String>,
    /// OAuth scopes to request. Defaults to ["openid", "email", "profile"].
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    /// Organization names to auto-add users to on login (with "read" role).
    #[serde(default)]
    pub auto_join_orgs: Vec<String>,
}

fn default_auth_external_url() -> String {
    "http://localhost:8080".to_string()
}
