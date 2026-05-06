use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Manages TLS certificate files on disk for Pingora's file-based TLS.
///
/// Pingora's rustls backend only supports loading certs from files, not
/// dynamic resolution. We write cert PEM files to a state directory and
/// point Pingora at the primary cert. For multi-domain SNI support, a
/// future version could switch to the OpenSSL backend's callback API.
pub struct CertFiles {
    state_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct CertEntry {
    pub domain: String,
    pub chain_pem: String,
    pub key_pem: String,
}

impl CertFiles {
    pub fn new(state_dir: &Path) -> Self {
        std::fs::create_dir_all(state_dir).expect("failed to create cert state directory");
        Self {
            state_dir: state_dir.to_owned(),
        }
    }

    /// Write the primary cert (used by Pingora's TLS listener).
    /// Returns (cert_path, key_path).
    pub fn write_primary(&self, chain_pem: &str, key_pem: &str) -> (PathBuf, PathBuf) {
        let cert_path = self.state_dir.join("primary.crt");
        let key_path = self.state_dir.join("primary.key");
        std::fs::write(&cert_path, chain_pem).expect("failed to write cert");
        std::fs::write(&key_path, key_pem).expect("failed to write key");
        (cert_path, key_path)
    }

    /// Write a self-signed cert as the primary cert.
    /// Returns (cert_path, key_path).
    pub fn write_self_signed(&self, domains: &[&str]) -> (PathBuf, PathBuf) {
        let key_pair = rcgen::KeyPair::generate().expect("keygen failed");
        let subject_alt_names: Vec<String> = domains.iter().map(|d| d.to_string()).collect();
        let mut params =
            rcgen::CertificateParams::new(subject_alt_names).expect("cert params failed");
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, domains[0]);

        let cert = params
            .self_signed(&key_pair)
            .expect("self-signed cert generation failed");

        self.write_primary(&cert.pem(), &key_pair.serialize_pem())
    }

    /// Update all certs from the server response. Writes the first cert
    /// as the primary cert for the TLS listener.
    pub fn update_from_entries(&self, entries: &[CertEntry]) -> bool {
        if entries.is_empty() {
            return false;
        }
        // Use the first entry as the primary cert
        let primary = &entries[0];
        self.write_primary(&primary.chain_pem, &primary.key_pem);
        tracing::info!(
            count = entries.len(),
            primary = %primary.domain,
            "updated certs on disk"
        );
        true
    }

    pub fn cert_path(&self) -> PathBuf {
        self.state_dir.join("primary.crt")
    }

    pub fn key_path(&self) -> PathBuf {
        self.state_dir.join("primary.key")
    }
}
