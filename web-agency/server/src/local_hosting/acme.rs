//! ACME certificate provisioning stub.

/// Request an SSL certificate for the given domains via ACME (Let's Encrypt).
pub fn request_certificate(domains: &[String]) -> Result<(), String> {
    // STUB: In production, call certbot or acme.sh
    tracing::info!(
        "STUB: would request ACME certificate for domains: {}",
        domains.join(", ")
    );
    Ok(())
}
