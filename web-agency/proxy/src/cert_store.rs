use arc_swap::ArcSwap;
use pingora::tls::pkey::{PKey, Private};
use pingora::tls::ssl::{NameType, SslRef};
use pingora::tls::x509::X509;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use pingora::listeners::TlsAccept;

/// A parsed certificate + private key pair ready for TLS handshake.
#[derive(Clone)]
pub struct CertKey {
    pub cert: X509,
    pub key: PKey<Private>,
}

impl CertKey {
    pub fn from_pem(chain_pem: &str, key_pem: &str) -> anyhow::Result<Self> {
        let cert = X509::from_pem(chain_pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("invalid cert PEM: {e}"))?;
        let key = PKey::private_key_from_pem(key_pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("invalid key PEM: {e}"))?;
        Ok(Self { cert, key })
    }
}

/// Dynamic certificate store that resolves certs by SNI hostname
/// during the TLS handshake callback. Uses ArcSwap for lock-free reads.
pub struct CertStore {
    certs: ArcSwap<HashMap<String, Arc<CertKey>>>,
    fallback: Arc<CertKey>,
}

impl CertStore {
    pub fn new(fallback: Arc<CertKey>) -> Self {
        Self {
            certs: ArcSwap::from_pointee(HashMap::new()),
            fallback,
        }
    }

    /// Replace all certs at once (called on reload).
    pub fn replace_all(&self, map: HashMap<String, Arc<CertKey>>) {
        self.certs.store(Arc::new(map));
    }

    pub fn has_cert(&self, domain: &str) -> bool {
        self.certs.load().contains_key(domain)
    }

    fn resolve(&self, sni: &str) -> Arc<CertKey> {
        let certs = self.certs.load();
        certs
            .get(sni)
            .cloned()
            .unwrap_or_else(|| self.fallback.clone())
    }
}

/// Wrapper so we can put `Arc<CertStore>` into `Box<dyn TlsAccept>` while
/// still sharing the store with the sync task.
pub struct CertStoreCallback(pub Arc<CertStore>);

#[async_trait::async_trait]
impl TlsAccept for CertStoreCallback {
    async fn certificate_callback(&self, ssl: &mut SslRef) {
        let sni = ssl
            .servername(NameType::HOST_NAME)
            .unwrap_or("")
            .to_lowercase();

        let ck = self.0.resolve(&sni);

        if let Err(e) = ssl.set_certificate(&ck.cert) {
            tracing::error!(sni = %sni, "failed to set certificate: {e}");
            return;
        }
        if let Err(e) = ssl.set_private_key(&ck.key) {
            tracing::error!(sni = %sni, "failed to set private key: {e}");
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CertEntry {
    pub domain: String,
    pub chain_pem: String,
    pub key_pem: String,
}
