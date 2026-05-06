use std::sync::Arc;

use crate::cert_store::CertKey;

/// Generate a self-signed certificate for the given domains.
/// Used as a fallback before ACME certs are available.
pub fn generate(domains: &[&str]) -> anyhow::Result<Arc<CertKey>> {
    let subject_alt_names: Vec<String> = domains.iter().map(|d| d.to_string()).collect();
    let key_pair = rcgen::KeyPair::generate()?;
    let mut params = rcgen::CertificateParams::new(subject_alt_names)?;
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, domains[0]);

    let cert = params.self_signed(&key_pair)?;

    let ck = CertKey::from_pem(&cert.pem(), &key_pair.serialize_pem())?;
    Ok(Arc::new(ck))
}
