//! Spaceship API types derived from the OpenAPI specification.

use serde::{Deserialize, Serialize};

// ── Async Operations ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncOperation {
    pub status: String, // pending, success, failed
    #[serde(rename = "type")]
    pub op_type: Option<String>,
    pub details: Option<serde_json::Value>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "modifiedAt")]
    pub modified_at: Option<String>,
}

// ── Contacts ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactDetails {
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub address1: String,
    pub city: String,
    pub country: String, // ISO 3166-1 alpha-2
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<String>,
    #[serde(rename = "stateProvince", skip_serializing_if = "Option::is_none")]
    pub state_province: Option<String>,
    #[serde(rename = "postalCode", skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactSaveResponse {
    #[serde(rename = "contactId")]
    pub contact_id: String,
}

// ── Domain Availability ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAvailability {
    pub domain: Option<String>,
    pub status: Option<String>, // available, taken, invalidDomainName, tldNotSupported
    #[serde(rename = "premiumPricing", default)]
    pub premium_pricing: Vec<PremiumPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumPrice {
    pub operation: String, // register, transfer, renew, restore
    pub price: f64,
    pub currency: String,
}

// ── Domain Management ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainListResponse {
    pub items: Vec<DomainInfo>,
    #[serde(rename = "totalCount")]
    pub total_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    pub name: String,
    #[serde(rename = "unicodeName")]
    pub unicode_name: Option<String>,
    #[serde(rename = "isPremium")]
    pub is_premium: Option<bool>,
    #[serde(rename = "autoRenew")]
    pub auto_renew: Option<bool>,
    #[serde(rename = "registrationDate")]
    pub registration_date: Option<String>,
    #[serde(rename = "expirationDate")]
    pub expiration_date: Option<String>,
    #[serde(rename = "lifecycleStatus")]
    pub lifecycle_status: Option<String>, // creating, registered, grace1, grace2, redemption
    pub nameservers: Option<NameserverConfig>,
    pub contacts: Option<DomainContacts>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    #[serde(rename = "autoRenew")]
    pub auto_renew: bool,
    pub years: u32,
    #[serde(rename = "privacyProtection")]
    pub privacy_protection: PrivacyProtection,
    pub contacts: DomainContacts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyProtection {
    pub level: String, // public, high
    #[serde(rename = "userConsent")]
    pub user_consent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainContacts {
    pub registrant: String, // contactId
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<String>,
}

// ── Nameservers ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameserverConfig {
    pub provider: String, // basic, custom
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
}

// ── DNS Records ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecordListResponse {
    pub items: Vec<DnsRecord>,
    #[serde(rename = "totalCount")]
    pub total_count: Option<u32>,
}

/// DNS record. The fields vary by record type, so we use serde_json::Value
/// for type-specific fields beyond the common ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    #[serde(rename = "type")]
    pub record_type: String, // A, AAAA, CNAME, ALIAS, MX, NS, TXT, SRV, CAA, PTR, TLSA, SVCB, HTTPS
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    // Type-specific fields — flatten all possible fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>, // A, AAAA
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cname: Option<String>, // CNAME
    #[serde(rename = "aliasName", skip_serializing_if = "Option::is_none")]
    pub alias_name: Option<String>, // ALIAS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>, // MX
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preference: Option<u16>, // MX
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nameserver: Option<String>, // NS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>, // TXT
}

// ── Transfer ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCodeResponse {
    #[serde(rename = "authCode")]
    pub auth_code: String,
}
