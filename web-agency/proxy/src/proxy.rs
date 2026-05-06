use arc_swap::ArcSwap;
use async_trait::async_trait;
use pingora::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

/// The Pingora HTTP proxy that routes requests by Host header.
pub struct WebAgencyProxy {
    pub routes: Arc<ArcSwap<HashMap<String, SocketAddr>>>,
}

#[async_trait]
impl ProxyHttp for WebAgencyProxy {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let host = extract_host(session);

        let routes = self.routes.load();
        if let Some(addr) = routes.get(&host) {
            let peer = Box::new(HttpPeer::new(*addr, false, String::new()));
            Ok(peer)
        } else {
            tracing::debug!(host = %host, "no route found");
            Err(pingora::Error::because(
                pingora::ErrorType::HTTPStatus(404),
                "no route for this host",
                pingora::Error::new(pingora::ErrorType::InternalError),
            ))
        }
    }
}

/// Extract the hostname from a request, handling both HTTP/1.1 Host header
/// and HTTP/2 :authority pseudo-header. Strips port if present.
fn extract_host(session: &Session) -> String {
    let raw = session
        .req_header()
        .headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .or_else(|| session.req_header().uri.host())
        .unwrap_or("");
    // Strip port
    raw.split(':').next().unwrap_or(raw).to_lowercase()
}

/// Build the Pingora HTTP proxy service with TLS from cert files on disk.
pub fn build_service(
    server_conf: &Arc<pingora::server::configuration::ServerConf>,
    cfg: &crate::config::ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, SocketAddr>>>,
    cert_files: &crate::cert_store::CertFiles,
) -> pingora::services::listening::Service<pingora::proxy::HttpProxy<WebAgencyProxy>> {
    let proxy = WebAgencyProxy { routes };

    let mut svc = http_proxy_service(server_conf, proxy);

    // HTTP listener (for redirect to HTTPS — handled in upstream_request_filter)
    svc.add_tcp(&cfg.http_addr);

    // HTTPS listener with file-based cert
    let cert_path = cert_files.cert_path();
    let key_path = cert_files.key_path();
    let mut tls_settings = pingora::listeners::tls::TlsSettings::intermediate(
        cert_path.to_str().unwrap(),
        key_path.to_str().unwrap(),
    )
    .expect("failed to create TLS settings");
    tls_settings.enable_h2();
    svc.add_tls_with_settings(&cfg.https_addr, None, tls_settings);

    svc
}
