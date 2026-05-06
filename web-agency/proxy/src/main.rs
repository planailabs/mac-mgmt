mod cert_store;
mod config;
mod proxy;
mod self_signed;
mod sync;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = config::load();
    tracing::info!(
        agency_domain = %cfg.agency_domain,
        http = %cfg.http_addr,
        https = %cfg.https_addr,
        "starting web-agency-proxy"
    );

    // Set up cert files directory
    let state_dir = std::path::Path::new(&cfg.internal_token_path)
        .parent()
        .unwrap_or(std::path::Path::new("/var/lib/web-agency"));
    let cert_files = cert_store::CertFiles::new(&state_dir.join("certs"));

    // Generate self-signed fallback cert so TLS works immediately
    cert_files.write_self_signed(&[&cfg.agency_domain]);
    tracing::info!("generated self-signed fallback cert");

    // Initial load from server API (replaces self-signed if real certs exist)
    let routes = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
        std::collections::HashMap::new(),
    ));
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(sync::initial_load(cfg, &routes, &cert_files));
    }

    // Set up Pingora
    let mut server = pingora::prelude::Server::new(None).unwrap();
    server.bootstrap();

    let conf = server.configuration.clone();
    let proxy_svc = proxy::build_service(&conf, cfg, routes.clone(), &cert_files);
    server.add_service(proxy_svc);

    // Spawn SSE sync as a background service
    let sync_svc = sync::build_service(cfg.clone(), routes);
    server.add_service(sync_svc);

    server.run_forever();
}
