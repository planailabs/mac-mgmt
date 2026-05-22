//! TLS listener with optional client certificate extraction.
//!
//! The relay serves HTTPS directly using rustls. Client certificates are
//! requested but not required at the TLS layer — authorization is by
//! fingerprint allowlist checked per-route.

use std::io;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt;

/// Information extracted from a client's TLS certificate.
#[derive(Debug, Clone)]
pub struct ClientCertInfo {
    /// SHA-256 fingerprint of the DER-encoded certificate, e.g. `sha256:ab12cd34...`
    pub fingerprint_sha256: String,
    /// Certificate subject common name (best-effort).
    pub subject: String,
}

// ── serve_tls ───────────────────────────────────────────────────────

/// Accept TLS connections and serve them with the given axum router.
/// Extracts client cert info and peer address, injecting them as request
/// extensions before passing to axum.
pub async fn serve_tls(
    listener: TcpListener,
    acceptor: TlsAcceptor,
    app: axum::Router,
) -> io::Result<()> {
    use hyper_util::rt::TokioIo;

    loop {
        let (tcp_stream, peer_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("TCP accept error: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };

        let acceptor = acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            tracing::debug!("TCP accepted from {peer_addr}, starting TLS handshake");
            let tls_stream = match acceptor.accept(tcp_stream).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!("TLS handshake failed from {peer_addr}: {e}");
                    return;
                }
            };

            // Extract client cert info before handing off to hyper/axum.
            let (_, server_conn) = tls_stream.get_ref();
            let cert_info = extract_client_cert(server_conn);
            let alpn = server_conn
                .alpn_protocol()
                .map(|p| String::from_utf8_lossy(p).to_string());
            tracing::debug!(
                %peer_addr,
                has_client_cert = cert_info.is_some(),
                ?alpn,
                "TLS handshake complete"
            );

            let io = TokioIo::new(tls_stream);

            // Service that injects cert info + peer addr into request
            // extensions, then delegates to the axum router.
            let hyper_service = hyper::service::service_fn(
                move |req: hyper::Request<hyper::body::Incoming>| {
                    let app = app.clone();
                    let info = cert_info.clone();
                    async move {
                        let (parts, body) = req.into_parts();
                        let body = axum::body::Body::new(body);
                        let mut req = axum::http::Request::from_parts(parts, body);

                        if let Some(info) = info {
                            req.extensions_mut().insert(info);
                        }
                        req.extensions_mut()
                            .insert(axum::extract::ConnectInfo(peer_addr));

                        let resp = app.oneshot(req).await;
                        resp.map_err(|e| match e {})
                    }
                },
            );

            tracing::debug!(%peer_addr, "starting hyper connection handler");
            if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                hyper_util::rt::TokioExecutor::new(),
            )
            .serve_connection_with_upgrades(io, hyper_service)
            .await
            {
                tracing::debug!("connection error from {peer_addr}: {e}");
            }
            tracing::debug!(%peer_addr, "hyper connection handler finished");
        });
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
