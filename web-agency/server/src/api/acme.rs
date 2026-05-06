//! ACME certificate issuance via Let's Encrypt with Cloudflare DNS-01 challenges.

use sqlx::PgPool;
use uuid::Uuid;

/// Issue a TLS certificate for the given domain using ACME DNS-01 challenge.
///
/// Looks up the domain's Cloudflare credentials in the database, creates a
/// DNS-01 TXT record via the CF API, completes the ACME challenge, and stores
/// the resulting certificate (encrypted) in the `certificates` table.
pub async fn issue_cert(pool: &PgPool, domain: &str) -> anyhow::Result<()> {
    // 1. Look up CF credentials for this domain
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT cloudflare_credential_id, cloudflare_zone_id \
         FROM domains \
         WHERE name = $1 \
           AND cloudflare_credential_id IS NOT NULL \
           AND cloudflare_zone_id IS NOT NULL",
    )
    .bind(domain)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("domain {domain} has no Cloudflare credentials"))?;

    let (cred_id, zone_id) = row;

    // 2. Mark as pending
    sqlx::query(
        "INSERT INTO certificates (domain, encrypted_chain, encrypted_key, issuer, not_before, not_after, acme_status) \
         VALUES ($1, '\\x00', '\\x00', 'acme', now(), now(), 'pending') \
         ON CONFLICT (domain) DO UPDATE SET acme_status = 'pending', last_error = NULL, updated_at = now()",
    )
    .bind(domain)
    .execute(pool)
    .await?;

    // 3. Run ACME flow
    let result = run_acme_flow(pool, domain, cred_id, &zone_id).await;

    match &result {
        Ok((chain_pem, key_pem)) => {
            // Parse cert to get validity dates
            let not_before = chrono::Utc::now();
            let not_after = not_before + chrono::Duration::days(90);

            let enc_chain = crate::crypto::encrypt(chain_pem.as_bytes())
                .map_err(|e| anyhow::anyhow!("encrypt chain: {e}"))?;
            let enc_key = crate::crypto::encrypt(key_pem.as_bytes())
                .map_err(|e| anyhow::anyhow!("encrypt key: {e}"))?;

            sqlx::query(
                "UPDATE certificates SET \
                    encrypted_chain = $2, encrypted_key = $3, \
                    issuer = 'letsencrypt', not_before = $4, not_after = $5, \
                    acme_status = 'valid', last_error = NULL, updated_at = now() \
                 WHERE domain = $1",
            )
            .bind(domain)
            .bind(&enc_chain)
            .bind(&enc_key)
            .bind(not_before)
            .bind(not_after)
            .execute(pool)
            .await?;
        }
        Err(e) => {
            sqlx::query(
                "UPDATE certificates SET acme_status = 'failed', last_error = $2, updated_at = now() WHERE domain = $1",
            )
            .bind(domain)
            .bind(e.to_string())
            .execute(pool)
            .await?;
        }
    }

    result.map(|_| ())
}

async fn run_acme_flow(
    pool: &PgPool,
    domain: &str,
    cred_id: Uuid,
    zone_id: &str,
) -> anyhow::Result<(String, String)> {
    use instant_acme::{
        Account, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewAccount,
        NewOrder, OrderStatus,
    };

    let cfg = crate::config::config();
    let contact_email = cfg
        .proxy
        .as_ref()
        .and_then(|p| p.acme_email.clone())
        .unwrap_or_else(|| format!("admin@{domain}"));

    // Create ACME account
    let (account, _creds) = Account::create(
        &NewAccount {
            contact: &[&format!("mailto:{contact_email}")],
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        LetsEncrypt::Production.url(),
        None,
    )
    .await?;

    // Create order
    let mut order = account
        .new_order(&NewOrder {
            identifiers: &[Identifier::Dns(domain.to_string())],
        })
        .await?;

    // Get authorization and DNS-01 challenge
    let authorizations = order.authorizations().await?;
    let authz = authorizations
        .first()
        .ok_or_else(|| anyhow::anyhow!("no authorizations returned"))?;

    if !matches!(authz.status, AuthorizationStatus::Pending) {
        anyhow::bail!("authorization not pending: {:?}", authz.status);
    }

    let challenge = authz
        .challenges
        .iter()
        .find(|c| c.r#type == ChallengeType::Dns01)
        .ok_or_else(|| anyhow::anyhow!("no DNS-01 challenge found"))?;

    let dns_value = order.key_authorization(challenge).dns_value();

    // Create TXT record via Cloudflare API
    let cf_client = crate::credentials::cf_client(pool, cred_id).await?;
    let txt_name = format!("_acme-challenge.{domain}");

    tracing::info!(domain, txt_name = %txt_name, "creating DNS-01 TXT record");

    // Delete any existing _acme-challenge records first
    let existing = cf_client.list_dns_records(zone_id).await?;
    for rec in &existing {
        if rec.name == txt_name && rec.record_type == "TXT" {
            let _ = cf_client.delete_dns_record(zone_id, &rec.id).await;
        }
    }

    let record = cloudflare_api::compat::CreateDnsRecord {
        record_type: "TXT".to_string(),
        name: txt_name.clone(),
        content: Some(dns_value),
        ttl: Some(60),
        proxied: Some(false),
        priority: None,
        data: None,
        comment: Some("ACME DNS-01 challenge".to_string()),
    };
    let created_record = cf_client.create_dns_record(zone_id, &record).await?;

    // Wait for DNS propagation
    tracing::info!(domain, "waiting 15s for DNS propagation");
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;

    // Tell ACME server the challenge is ready
    order.set_challenge_ready(&challenge.url).await?;

    // Poll for order to become ready
    let mut tries = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let state = order.refresh().await?;
        if matches!(state.status, OrderStatus::Ready | OrderStatus::Valid) {
            break;
        }
        if matches!(state.status, OrderStatus::Invalid) {
            // Clean up TXT record
            let _ = cf_client
                .delete_dns_record(zone_id, &created_record.id)
                .await;
            anyhow::bail!("ACME order became invalid");
        }
        tries += 1;
        if tries > 20 {
            let _ = cf_client
                .delete_dns_record(zone_id, &created_record.id)
                .await;
            anyhow::bail!("ACME order timed out after 60s");
        }
    }

    // Finalize: generate CSR and get certificate
    let mut params = rcgen::CertificateParams::new(vec![domain.to_string()])?;
    params.distinguished_name = rcgen::DistinguishedName::new();
    let key_pair = rcgen::KeyPair::generate()?;
    let csr = params.serialize_request(&key_pair)?;

    order.finalize(csr.der()).await?;

    // Poll for certificate
    let cert_chain_pem = loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let state = order.refresh().await?;
        if matches!(state.status, OrderStatus::Valid) {
            break order
                .certificate()
                .await?
                .ok_or_else(|| anyhow::anyhow!("no certificate returned"))?;
        }
        if matches!(state.status, OrderStatus::Invalid) {
            let _ = cf_client
                .delete_dns_record(zone_id, &created_record.id)
                .await;
            anyhow::bail!("order became invalid during finalization");
        }
    };

    // Clean up TXT record
    let _ = cf_client
        .delete_dns_record(zone_id, &created_record.id)
        .await;

    tracing::info!(domain, "ACME cert issued successfully");

    Ok((cert_chain_pem, key_pair.serialize_pem()))
}
