use anyhow::{Context, Result};

/// Build a reqwest client that binds to IPv6 localhost with a 5-second timeout.
/// Used by CLI subcommands (`status`, `logs`, `sync`) to talk to the local
/// daemon metrics server.
pub fn build() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .local_address(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST))
        .build()
        .context("failed to build HTTP client")
}

/// Map a reqwest connection error to a user-friendly message.
pub fn map_connect_error(e: reqwest::Error) -> anyhow::Error {
    if e.is_connect() {
        anyhow::anyhow!("daemon not running or metrics port differs")
    } else {
        anyhow::anyhow!("{e}")
    }
}
