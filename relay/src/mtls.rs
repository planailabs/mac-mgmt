//! TLS listener with optional client certificate extraction.
//!
//! Implements axum's `Listener` trait so `axum::serve()` works directly
//! with TLS — no manual hyper plumbing needed. Client certificates are
//! requested but not required; cert info is extracted during accept and
//! made available via `ConnectInfo<TlsConnectInfo>`.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// Information extracted from a client's TLS certificate.
#[derive(Debug, Clone)]
pub struct ClientCertInfo {
    /// SHA-256 fingerprint of the DER-encoded certificate, e.g. `sha256:ab12cd34...`
    pub fingerprint_sha256: String,
    /// Certificate subject common name (best-effort).
    pub subject: String,
}

// ── TLS Listener (implements axum::serve::Listener) ─────────────────

/// A TLS listener that wraps a `TcpListener` + `TlsAcceptor`.
/// Implements axum's `Listener` trait so it plugs directly into `axum::serve()`.
pub struct TlsListener {
    tcp: TcpListener,
    acceptor: TlsAcceptor,
}

impl TlsListener {
    pub fn new(tcp: TcpListener, acceptor: TlsAcceptor) -> Self {
        Self { tcp, acceptor }
    }
}

/// Address info returned by `TlsListener::accept()`, carrying both the
/// remote socket address and any extracted client certificate info.
#[derive(Debug, Clone)]
pub struct TlsAddr {
    pub remote_addr: SocketAddr,
    pub cert_info: Option<ClientCertInfo>,
}

impl axum::serve::Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = TlsAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (tcp, addr) = match self.tcp.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::warn!("TCP accept error: {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            };

            match self.acceptor.accept(tcp).await {
                Ok(tls) => {
                    let (_, server_conn) = tls.get_ref();
                    let cert_info = extract_client_cert(server_conn);
                    return (
                        tls,
                        TlsAddr {
                            remote_addr: addr,
                            cert_info,
                        },
                    );
                }
                Err(e) => {
                    tracing::debug!("TLS handshake failed from {addr}: {e}");
                    continue;
                }
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.tcp.local_addr().map(|a| TlsAddr {
            remote_addr: a,
            cert_info: None,
        })
    }
}

// ── ConnectInfo for cert extraction ─────────────────────────────────

/// Connection info extracted from a TLS connection, available via
/// `ConnectInfo<TlsConnectInfo>` in axum handlers.
#[derive(Debug, Clone)]
pub struct TlsConnectInfo {
    pub remote_addr: SocketAddr,
    pub cert_info: Option<ClientCertInfo>,
}

impl axum::extract::connect_info::Connected<axum::serve::IncomingStream<'_, TlsListener>>
    for TlsConnectInfo
{
    fn connect_info(stream: axum::serve::IncomingStream<'_, TlsListener>) -> Self {
        let addr = stream.remote_addr();
        TlsConnectInfo {
            remote_addr: addr.remote_addr,
            cert_info: addr.cert_info.clone(),
        }
    }
}

// ── TLS config builders ─────────────────────────────────────────────

/// Build a `rustls::ServerConfig` that requests (but does not require)
/// client certificates.
pub fn build_tls_config(
    cert_chain: Vec<rustls::pki_types::CertificateDer<'static>>,
    private_key: rustls::pki_types::PrivateKeyDer<'static>,
    _client_ca_path: Option<&str>,
) -> Result<rustls::ServerConfig, rustls::Error> {
    let client_verifier = Arc::new(AcceptAnyClientCert);

    let mut config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(cert_chain, private_key)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

/// Load TLS certificate chain and private key from PEM files.
pub fn load_certs_and_key(
    cert_path: &str,
    key_path: &str,
) -> anyhow::Result<(
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
)> {
    let cert_pem = std::fs::read(cert_path)
        .map_err(|e| anyhow::anyhow!("failed to read TLS cert from {cert_path}: {e}"))?;
    let key_pem = std::fs::read(key_path)
        .map_err(|e| anyhow::anyhow!("failed to read TLS key from {key_path}: {e}"))?;

    let certs: Vec<_> = rustls_pemfile::certs(&mut &cert_pem[..])
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("failed to parse TLS cert: {e}"))?;
    if certs.is_empty() {
        anyhow::bail!("no certificates found in {cert_path}");
    }

    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .map_err(|e| anyhow::anyhow!("failed to parse TLS key: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in {key_path}"))?;

    Ok((certs, key))
}

/// Generate a self-signed certificate for development use.
pub fn generate_self_signed() -> anyhow::Result<(
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
)> {
    use rcgen::{CertifiedKey, generate_simple_self_signed};

    let subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];

    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names)
        .map_err(|e| anyhow::anyhow!("failed to generate self-signed cert: {e}"))?;

    let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(key_pair.serialize_der())
        .map_err(|e| anyhow::anyhow!("failed to convert key: {e}"))?;

    Ok((vec![cert_der], key_der))
}

/// Create a `TlsAcceptor` from the server config.
pub fn make_acceptor(config: rustls::ServerConfig) -> TlsAcceptor {
    TlsAcceptor::from(Arc::new(config))
}

// ── Cert extraction ─────────────────────────────────────────────────

/// Extract `ClientCertInfo` from a rustls server connection.
fn extract_client_cert(conn: &rustls::ServerConnection) -> Option<ClientCertInfo> {
    let certs = conn.peer_certificates()?;
    let cert_der = certs.first()?;

    let fingerprint = {
        let hash = Sha256::digest(cert_der.as_ref());
        format!("sha256:{}", hex::encode(hash))
    };

    let subject = match x509_parser::parse_x509_certificate(cert_der.as_ref()) {
        Ok((_rem, parsed)) => parsed
            .subject()
            .iter_common_name()
            .next()
            .and_then(|cn| cn.as_str().ok())
            .unwrap_or("")
            .to_string(),
        Err(_) => String::new(),
    };

    Some(ClientCertInfo {
        fingerprint_sha256: fingerprint,
        subject,
    })
}

// ── Client cert verifier that accepts anything ──────────────────────

#[derive(Debug)]
struct AcceptAnyClientCert;

impl rustls::server::danger::ClientCertVerifier for AcceptAnyClientCert {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        Ok(rustls::server::danger::ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }

    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        false
    }
}
