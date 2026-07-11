//! TLS listener with optional client certificate extraction.
//!
//! The relay serves HTTPS directly using rustls. Client certificates are
//! requested but not required at the TLS layer — authorization is by
//! fingerprint allowlist checked per-route.

use std::io;
use std::sync::Arc;

use base64::Engine;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tower::ServiceExt;

/// Which TLS config class a connection was accepted with, based on SNI.
/// Injected as a request extension so the HTTP layer can reject requests
/// that arrive on the wrong connection class (HTTP/2 coalescing) with 421.
#[derive(Debug, Clone, Copy)]
pub struct TlsSniClass {
    pub is_proxy_subdomain: bool,
}

/// Information extracted from a client's TLS certificate.
#[derive(Debug, Clone)]
pub struct ClientCertInfo {
    /// SHA-256 fingerprint of the DER-encoded certificate, e.g. `sha256:ab12cd34...`
    pub fingerprint_sha256: String,
    /// Certificate subject common name (best-effort).
    pub subject: String,
    /// PEM-encoded certificate.
    pub certificate_pem: String,
}

// ── serve_tls ───────────────────────────────────────────────────────

/// Accept TLS connections and serve them with the given axum router.
/// Extracts client cert info and peer address, injecting them as request
/// extensions before passing to axum.
///
/// Per-SNI config selection: connections whose SNI falls under
/// `.{proxy_hostname}` (wildcard tunnel subdomains) get `quiet_config`, which
/// still accepts client certificates but advertises a decoy CA hint so
/// browsers never show the "select a certificate" picker. Everything else
/// (the main relay domain) gets `main_config`, which sends no CA hints and
/// therefore lets the browser offer its client certificates.
pub async fn serve_tls(
    listener: TcpListener,
    main_config: Arc<rustls::ServerConfig>,
    quiet_config: Arc<rustls::ServerConfig>,
    proxy_hostname: Option<String>,
    app: axum::Router,
) -> io::Result<()> {
    use hyper_util::rt::TokioIo;

    let proxy_suffix = proxy_hostname.map(|h| format!(".{h}"));

    loop {
        let (tcp_stream, peer_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("TCP accept error: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
        };

        let main_config = main_config.clone();
        let quiet_config = quiet_config.clone();
        let proxy_suffix = proxy_suffix.clone();
        let app = app.clone();

        tokio::spawn(async move {
            tracing::debug!("TCP accepted from {peer_addr}, starting TLS handshake");
            let start = match tokio_rustls::LazyConfigAcceptor::new(
                rustls::server::Acceptor::default(),
                tcp_stream,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!("TLS ClientHello read failed from {peer_addr}: {e}");
                    return;
                }
            };

            let is_proxy_subdomain =
                match (proxy_suffix.as_deref(), start.client_hello().server_name()) {
                    (Some(suffix), Some(sni)) => sni.ends_with(suffix),
                    _ => false,
                };
            let config = if is_proxy_subdomain {
                quiet_config
            } else {
                main_config
            };

            let tls_stream = match start.into_stream(config).await {
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
            let hyper_service =
                hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
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
                            .insert(TlsSniClass { is_proxy_subdomain });
                        req.extensions_mut()
                            .insert(axum::extract::ConnectInfo(peer_addr));

                        let resp = app.oneshot(req).await;
                        resp.map_err(|e| match e {})
                    }
                });

            tracing::debug!(%peer_addr, "starting hyper connection handler");
            if let Err(e) =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
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

/// Build the pair of `rustls::ServerConfig`s used by [`serve_tls`].
///
/// Both request (but do not require) client certificates and accept any
/// certificate at the TLS layer. The difference is the CA hints sent in the
/// CertificateRequest:
/// - main: no hints — browsers show their certificate picker (used on the
///   main relay domain, e.g. for /cert-login).
/// - quiet: a decoy CA hint that matches no real certificate — browsers
///   silently continue without a certificate (no picker popup), while
///   non-browser clients (curl, reqwest, daemons) still send their
///   configured certificate regardless of hints.
pub fn build_tls_configs(
    cert_chain: Vec<rustls::pki_types::CertificateDer<'static>>,
    private_key: rustls::pki_types::PrivateKeyDer<'static>,
) -> Result<(Arc<rustls::ServerConfig>, Arc<rustls::ServerConfig>), rustls::Error> {
    let build = |verifier: Arc<dyn rustls::server::danger::ClientCertVerifier>|
     -> Result<Arc<rustls::ServerConfig>, rustls::Error> {
        let mut config = rustls::ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(cert_chain.clone(), private_key.clone_key())?;
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(Arc::new(config))
    };

    let main = build(Arc::new(AcceptAnyClientCert { hints: vec![] }))?;
    let quiet = build(Arc::new(AcceptAnyClientCert {
        hints: vec![rustls::DistinguishedName::from(decoy_ca_hint())],
    }))?;
    Ok((main, quiet))
}

/// DER-encoded X.501 Name `CN=plan-ai-relay-no-prompt`, used as a CA hint
/// that intentionally matches no real client certificate issuer.
fn decoy_ca_hint() -> Vec<u8> {
    let cn = b"plan-ai-relay-no-prompt";
    let mut atv = vec![0x06, 0x03, 0x55, 0x04, 0x03]; // OID 2.5.4.3 (commonName)
    atv.push(0x0c); // UTF8String
    atv.push(cn.len() as u8);
    atv.extend_from_slice(cn);

    let mut seq_atv = vec![0x30, atv.len() as u8];
    seq_atv.extend_from_slice(&atv);
    let mut set_rdn = vec![0x31, seq_atv.len() as u8];
    set_rdn.extend_from_slice(&seq_atv);
    let mut name = vec![0x30, set_rdn.len() as u8];
    name.extend_from_slice(&set_rdn);
    name
}

#[cfg(test)]
mod tests {
    #[test]
    fn decoy_hint_is_valid_der_name() {
        use x509_parser::prelude::FromDer;
        let der = super::decoy_ca_hint();
        let (rem, name) = x509_parser::x509::X509Name::from_der(&der).unwrap();
        assert!(rem.is_empty());
        assert_eq!(name.to_string(), "CN=plan-ai-relay-no-prompt");
    }
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

    let certificate_pem = format!(
        "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----",
        base64::engine::general_purpose::STANDARD.encode(cert_der.as_ref()),
    );

    Some(ClientCertInfo {
        fingerprint_sha256: fingerprint,
        subject,
        certificate_pem,
    })
}

// ── Client cert verifier that accepts anything ──────────────────────

#[derive(Debug)]
struct AcceptAnyClientCert {
    /// CA subject hints advertised in the CertificateRequest. Empty = browsers
    /// offer all client certs (picker); a decoy hint = browsers stay silent.
    hints: Vec<rustls::DistinguishedName>,
}

impl rustls::server::danger::ClientCertVerifier for AcceptAnyClientCert {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &self.hints
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
