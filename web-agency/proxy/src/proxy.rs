use arc_swap::ArcSwap;
use async_trait::async_trait;
use pingora::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

/// A route target — either direct (local) or relay (via mac-mgmt relay).
#[derive(Debug, Clone)]
pub enum Route {
    /// Direct TCP upstream (local webspace, agency server).
    Direct(SocketAddr),
    /// Relay tunnel — proxy via HTTPS to relay server with token auth.
    Relay {
        addr: SocketAddr,
        /// Full relay URL (e.g. https://abc123-ollama.relay.plan.ai)
        url: String,
        /// Relay hostname for the Host header
        relay_host: String,
        /// Whether to use TLS to the relay
        tls: bool,
        /// SNI hostname for TLS
        sni: String,
        /// Proxy token to inject as X-Proxy-Token header
        proxy_token: String,
    },
}

/// Per-request context — carries relay info from upstream_peer to upstream_request_filter.
#[derive(Default)]
pub struct RequestCtx {
    relay: Option<RelayCtx>,
}

struct RelayCtx {
    host: String,
    proxy_token: String,
    path_prefix: String,
}

/// The Pingora HTTP proxy that routes requests by Host header.
pub struct WebAgencyProxy {
    pub routes: Arc<ArcSwap<HashMap<String, Route>>>,
}

#[async_trait]
impl ProxyHttp for WebAgencyProxy {
    type CTX = RequestCtx;

    fn new_ctx(&self) -> Self::CTX {
        RequestCtx::default()
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let host = extract_host(session);

        let routes = self.routes.load();
        match routes.get(&host) {
            Some(Route::Direct(addr)) => {
                Ok(Box::new(HttpPeer::new(*addr, false, String::new())))
            }
            Some(Route::Relay {
                addr,
                url,
                relay_host,
                tls,
                sni,
                proxy_token,
            }) => {
                // Store relay info in context for upstream_request_filter
                let path_prefix = url
                    .strip_prefix("https://")
                    .or_else(|| url.strip_prefix("http://"))
                    .and_then(|s| {
                        let after_host = s.find('/').map(|i| &s[i..]).unwrap_or("");
                        if after_host.is_empty() || after_host == "/" {
                            None
                        } else {
                            Some(after_host.trim_end_matches('/').to_string())
                        }
                    })
                    .unwrap_or_default();

                ctx.relay = Some(RelayCtx {
                    host: relay_host.clone(),
                    proxy_token: proxy_token.clone(),
                    path_prefix,
                });

                let mut peer = HttpPeer::new(*addr, *tls, sni.clone());
                // Override SNI for TLS connections
                if *tls {
                    peer.sni = sni.clone();
                }
                Ok(Box::new(peer))
            }
            None => {
                tracing::debug!(host = %host, "no route found");
                Err(pingora::Error::because(
                    pingora::ErrorType::HTTPStatus(404),
                    "no route for this host",
                    pingora::Error::new(pingora::ErrorType::InternalError),
                ))
            }
        }
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        if let Some(relay) = &ctx.relay {
            // Rewrite Host header to the relay hostname
            upstream_request
                .insert_header("host", &relay.host)
                .map_err(|e| {
                    pingora::Error::because(
                        pingora::ErrorType::InternalError,
                        "failed to set host header",
                        e,
                    )
                })?;

            // Inject proxy token
            upstream_request
                .insert_header("x-proxy-token", &relay.proxy_token)
                .map_err(|e| {
                    pingora::Error::because(
                        pingora::ErrorType::InternalError,
                        "failed to set proxy token header",
                        e,
                    )
                })?;

            // Prepend path prefix if the relay URL had a path
            if !relay.path_prefix.is_empty() {
                let original_path = upstream_request.uri.path().to_string();
                let query = upstream_request
                    .uri
                    .query()
                    .map(|q| format!("?{q}"))
                    .unwrap_or_default();
                let new_uri = format!("{}{}{}", relay.path_prefix, original_path, query);
                if let Ok(uri) = new_uri.parse() {
                    upstream_request.set_uri(uri);
                }
            }
        }
        Ok(())
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
    raw.split(':').next().unwrap_or(raw).to_lowercase()
}

/// Build the Pingora HTTP proxy service with BoringSSL TLS and dynamic SNI.
pub fn build_service(
    server_conf: &Arc<pingora::server::configuration::ServerConf>,
    cfg: &crate::config::ProxyConfig,
    routes: Arc<ArcSwap<HashMap<String, Route>>>,
    cert_store: Arc<crate::cert_store::CertStore>,
) -> pingora::services::listening::Service<pingora::proxy::HttpProxy<WebAgencyProxy>> {
    let proxy = WebAgencyProxy { routes };

    let mut svc = http_proxy_service(server_conf, proxy);

    svc.add_tcp(&cfg.http_addr);

    let callback = crate::cert_store::CertStoreCallback(cert_store);
    let mut tls_settings =
        pingora::listeners::tls::TlsSettings::with_callbacks(Box::new(callback))
            .expect("failed to create TLS settings");
    tls_settings.enable_h2();
    svc.add_tls_with_settings(&cfg.https_addr, None, tls_settings);

    svc
}
