//! Webspace-host endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub organization_name: String,
    pub folder_count: i64,
    pub hostname: Option<String>,
    pub has_changedetection: bool,
    pub has_missing_cname: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HostListInput {}

/// List webspace hosts the caller may see (admins: all; else their orgs').
#[api_mcp_dioxus_server(server = "list_hosts")]
pub async fn host_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    _input: HostListInput,
) -> Result<Vec<HostRow>, ApiError> {
    // hostname: all bound (domain, subdomain) FQDNs for the host, comma-joined.
    let hostname_subquery = "(SELECT string_agg(CASE WHEN s.name IS NOT NULL AND s.name != '@' \
            THEN s.name || '.' || d.name ELSE d.name END, ', ' ORDER BY d.name) \
         FROM webspace_host_domains whd \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE whd.webspace_host_id = h.id)";

    // has_missing_cname: at least one domain binding without a matching CNAME record.
    let missing_cname_subquery = "\
        EXISTS( \
            SELECT 1 FROM webspace_host_domains whd2 \
            JOIN domains d2 ON d2.id = whd2.domain_id \
            LEFT JOIN subdomains s2 ON s2.id = whd2.subdomain_id \
            WHERE whd2.webspace_host_id = h.id \
              AND NOT EXISTS( \
                  SELECT 1 FROM dns_records dr \
                  WHERE dr.domain_id = d2.id \
                    AND dr.record_type = 'CNAME' \
                    AND dr.name = COALESCE(s2.name, '@') \
              ) \
        )";

    type Row = (
        Uuid,
        String,
        String,
        String,
        i64,
        Option<String>,
        bool,
        bool,
    );
    let query = format!(
        "SELECT h.id, h.name, h.kind, o.name, \
         (SELECT count(*) FROM webspaces w WHERE w.webspace_host_id = h.id), \
         {hostname_subquery}, \
         h.changedetection_credential_id IS NOT NULL, \
         {missing_cname_subquery} \
         FROM webspace_hosts h JOIN organizations o ON o.id = h.organization_id"
    );

    let rows = match principal.read_filter() {
        None => {
            sqlx::query_as::<_, Row>(&format!("{query} ORDER BY h.name"))
                .fetch_all(pool)
                .await
        }
        Some(org_ids) => {
            sqlx::query_as::<_, Row>(&format!(
                "{query} WHERE h.organization_id = ANY($1) ORDER BY h.name"
            ))
            .bind(org_ids)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                name,
                kind,
                organization_name,
                folder_count,
                hostname,
                has_changedetection,
                has_missing_cname,
            )| HostRow {
                id,
                name,
                kind,
                organization_name,
                folder_count,
                hostname,
                has_changedetection,
                has_missing_cname,
            },
        )
        .collect())
}
