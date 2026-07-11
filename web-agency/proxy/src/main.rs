mod cert_store;
mod config;
mod proxy;
mod self_signed;
mod sync;

fn main() {
    // Install rustls crypto provider for reqwest (used by the sync client).
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = config::load();
    tracing::info!(
        agency_domain = %cfg.agency_domain,
        http = %cfg.http_addr,
        https = %cfg.https_addr,
        "starting web-agency-proxy"
    );

    // Generate self-signed fallback cert
    let fallback =
        self_signed::generate(&[&cfg.agency_domain]).expect("failed to generate self-signed cert");
    tracing::info!("generated self-signed fallback cert");

    // Build shared state
    let cert_store = std::sync::Arc::new(cert_store::CertStore::new(fallback));
    let routes = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
        std::collections::HashMap::<String, Vec<proxy::Folder>>::new(),
    ));

    // Note: we deliberately do NOT block on an initial route/cert fetch here.
    // Pingora starts immediately so it can accept connections and serve the
    // localized "starting up" page (empty route table) while the background sync
    // service performs the first load. TLS uses the self-signed fallback cert
    // until real certs arrive.

    // Set up Pingora
    let mut server = pingora::prelude::Server::new(None).unwrap();
    server.bootstrap();

    let conf = server.configuration.clone();
    let proxy_svc = proxy::build_service(&conf, cfg, routes.clone(), cert_store.clone());
    server.add_service(proxy_svc);

    // Spawn SSE sync as a background service
    let sync_svc = sync::build_service(cfg.clone(), routes, cert_store);
    server.add_service(sync_svc);

    server.run_forever();
}
