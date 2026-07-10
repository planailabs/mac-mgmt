use anyhow::{Context, Result, bail};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Parser)]
#[command(
    name = "relay-ssh",
    version,
    about = "SSH into mac-mgmt managed machines via relay"
)]
struct Cli {
    /// Instance ID or prefix to connect to
    instance_id: Option<String>,

    /// List active SSH targets
    #[arg(long)]
    list: bool,

    /// Relay URL (overrides config)
    #[arg(long)]
    relay: Option<String>,

    /// Auth token (overrides config)
    #[arg(long)]
    token: Option<String>,

    /// SSH user (default: current user)
    #[arg(short, long)]
    user: Option<String>,

    /// TLS client certificate (PEM) for cert-based auth via WebSocket
    #[arg(long)]
    cert: Option<String>,

    /// TLS client key (PEM) for cert-based auth via WebSocket
    #[arg(long)]
    key: Option<String>,

    /// Skip TLS certificate verification (DANGEROUS — only for a relay with a
    /// self-signed cert in local development). Never use against production.
    #[arg(long)]
    insecure: bool,
}

#[derive(Deserialize)]
struct Config {
    relay_url: Option<String>,
    token: Option<String>,
}

/// An SSH-capable daemon returned by the relay's `/api/ssh` endpoint.
#[derive(Deserialize)]
struct SshTarget {
    instance_id: String,
    cluster_name: Option<String>,
    agent_name: Option<String>,
    hostname: Option<String>,
    ssh_port: Option<u16>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/root"))
        .join("relay-ssh/config.toml")
}

fn load_config() -> Config {
    let path = config_path();
    if !path.exists() {
        return Config {
            relay_url: None,
            token: None,
        };
    }
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or(Config {
            relay_url: None,
            token: None,
        }),
        Err(_) => Config {
            relay_url: None,
            token: None,
        },
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    let cli = Cli::parse();
    let config = load_config();

    let relay_url = cli
        .relay
        .or_else(|| std::env::var("RELAY_URL").ok())
        .or(config.relay_url)
        .context("relay URL not configured (use --relay, RELAY_URL env, or config file)")?;

    // Cert-based mode: connect via WebSocket instead of SSH over TCP port.
    if let (Some(cert_path), Some(key_path)) = (&cli.cert, &cli.key) {
        let instance_id = cli
            .instance_id
            .as_deref()
            .context("instance ID required with --cert/--key")?;

        return connect_via_websocket(&relay_url, cert_path, key_path, instance_id, cli.insecure)
            .await;
    }

    let token = cli
        .token
        .or_else(|| std::env::var("RELAY_TOKEN").ok())
        .or(config.token)
        .context("token not configured (use --token, RELAY_TOKEN env, or config file)")?;

    let targets = fetch_ssh_targets(&relay_url, &token).await?;

    if cli.list {
        print_targets(&targets);
        return Ok(());
    }

    let target = match &cli.instance_id {
        Some(prefix) => {
            let matches: Vec<_> = targets
                .iter()
                .filter(|t| t.instance_id.starts_with(prefix.as_str()))
                .collect();
            match matches.len() {
                0 => bail!("no SSH target matching prefix '{prefix}'"),
                1 => matches[0],
                n => {
                    eprintln!("ambiguous prefix '{prefix}', matches {n} targets:");
                    for t in &matches {
                        eprintln!(
                            "  {}  {}",
                            &t.instance_id[..12.min(t.instance_id.len())],
                            t.agent_name.as_deref().unwrap_or("-")
                        );
                    }
                    bail!("use a longer prefix");
                }
            }
        }
        None => {
            if targets.len() == 1 {
                &targets[0]
            } else if targets.is_empty() {
                bail!("no active SSH targets");
            } else {
                eprintln!("multiple SSH targets active, specify an instance ID:");
                print_targets(&targets);
                bail!("specify an instance ID or prefix");
            }
        }
    };

    // If no TCP SSH port allocated, use WebSocket path via relay's /ssh endpoint.
    let Some(ssh_port) = target.ssh_port else {
        eprintln!(
            "No TCP SSH port allocated — connecting via WebSocket to {} ({})...",
            target.agent_name.as_deref().unwrap_or("-"),
            &target.instance_id[..12.min(target.instance_id.len())],
        );
        return connect_via_websocket_with_token(
            &relay_url,
            &token,
            &target.instance_id,
            cli.insecure,
        )
        .await;
    };

    let host = relay_url
        .strip_prefix("https://")
        .or_else(|| relay_url.strip_prefix("http://"))
        .unwrap_or(&relay_url)
        .split('/')
        .next()
        .unwrap_or(&relay_url)
        .split(':')
        .next()
        .unwrap_or(&relay_url);

    eprintln!(
        "Connecting to {} ({}) on {}:{}...",
        target.agent_name.as_deref().unwrap_or("-"),
        &target.instance_id[..12.min(target.instance_id.len())],
        host,
        ssh_port,
    );

    let mut cmd = Command::new("ssh");
    cmd.arg("-p").arg(ssh_port.to_string());

    if let Some(user) = &cli.user {
        cmd.arg(format!("{user}@{host}"));
    } else {
        cmd.arg(host);
    }

    let status = cmd.status().context("failed to exec ssh")?;
    std::process::exit(status.code().unwrap_or(1));
}

/// Connect via WebSocket using a Bearer token (no client cert needed).
/// The relay's /ssh endpoint accepts Bearer tokens as an auth alternative.
async fn connect_via_websocket_with_token(
    relay_url: &str,
    token: &str,
    instance_id: &str,
    insecure: bool,
) -> Result<()> {
    let ws_url = relay_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    let ws_url = format!("{ws_url}/ssh/{instance_id}/ws");

    // Build WS request with Bearer token in header.
    let request = http::Request::builder()
        .uri(&ws_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header(
            "Host",
            http::Uri::try_from(&ws_url)
                .ok()
                .and_then(|u| u.host().map(String::from))
                .unwrap_or_default(),
        )
        .body(())
        .context("failed to build WS request")?;

    let builder = rustls::ClientConfig::builder();
    let tls_config = if insecure {
        eprintln!("WARNING: --insecure: TLS certificate verification disabled");
        builder
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(AcceptAnyCert))
            .with_no_client_auth()
    } else {
        builder
            .with_root_certificates(native_root_store()?)
            .with_no_client_auth()
    };
    let connector = tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(tls_config));

    let (ws_stream, _resp) =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(connector))
            .await
            .context("WebSocket connection failed")?;

    bridge_terminal_websocket(ws_stream).await
}

async fn bridge_terminal_websocket<S>(
    ws_stream: tokio_tungstenite::WebSocketStream<S>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let stdin_to_ws = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 8192];
        loop {
            let n = stdin.read(&mut buf).await?;
            if n == 0 {
                return Ok::<(), anyhow::Error>(());
            }
            ws_tx
                .send(tokio_tungstenite::tungstenite::Message::Binary(
                    buf[..n].to_vec().into(),
                ))
                .await?;
        }
    });

    let ws_to_stdout = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let mut stderr = tokio::io::stderr();
        while let Some(msg) = ws_rx.next().await {
            let msg = match msg {
                Ok(msg) => msg,
                Err(err) => {
                    eprintln!("WebSocket closed: {err}");
                    break;
                }
            };
            match msg {
                tokio_tungstenite::tungstenite::Message::Binary(data) => {
                    stdout.write_all(&data).await?;
                    stdout.flush().await?;
                }
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    stderr.write_all(text.as_bytes()).await?;
                    stderr.write_all(b"\n").await?;
                    stderr.flush().await?;
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    let output_result = ws_to_stdout.await?;
    stdin_to_ws.abort();
    output_result
}

async fn fetch_ssh_targets(relay_url: &str, token: &str) -> Result<Vec<SshTarget>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{relay_url}/api/ssh"))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach relay")?;

    if !resp.status().is_success() {
        bail!("relay returned {}", resp.status());
    }

    resp.json().await.context("failed to parse SSH target list")
}

/// Connect via WebSocket with TLS client certificate.
/// The relay endpoint already terminates SSH and exposes a shell stream over WS,
/// so the CLI bridges terminal stdin/stdout directly instead of spawning ssh.
async fn connect_via_websocket(
    relay_url: &str,
    cert_path: &str,
    key_path: &str,
    instance_id: &str,
    insecure: bool,
) -> Result<()> {
    let cert_pem = std::fs::read(cert_path)
        .with_context(|| format!("failed to read cert from {cert_path}"))?;
    let key_pem =
        std::fs::read(key_path).with_context(|| format!("failed to read key from {key_path}"))?;

    let certs: Vec<_> = rustls_pemfile::certs(&mut &cert_pem[..])
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to parse client cert")?;
    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .context("failed to parse client key")?
        .context("no private key found")?;

    let builder = rustls::ClientConfig::builder();
    let mut tls_config = if insecure {
        eprintln!("WARNING: --insecure: TLS certificate verification disabled");
        builder
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(AcceptAnyCert))
            .with_client_auth_cert(certs, key)
            .context("failed to configure client cert")?
    } else {
        builder
            .with_root_certificates(native_root_store()?)
            .with_client_auth_cert(certs, key)
            .context("failed to configure client cert")?
    };
    tls_config.alpn_protocols = vec![b"http/1.1".to_vec()];

    let connector = tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(tls_config));
    let ws_url = relay_url
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    let ws_url = format!("{ws_url}/ssh/{instance_id}/ws");

    eprintln!("Connecting to {ws_url} with client certificate...");
    let (ws_stream, _resp) =
        tokio_tungstenite::connect_async_tls_with_config(&ws_url, None, false, Some(connector))
            .await
            .context("WebSocket connection failed")?;
    eprintln!("Connected. Bridging terminal stream...");

    bridge_terminal_websocket(ws_stream).await
}

/// Build a root certificate store from the OS trust store. Operators who front
/// the relay with an internal CA install that CA into the system store, so this
/// covers both public and internal PKI without disabling verification.
fn native_root_store() -> Result<rustls::RootCertStore> {
    let mut store = rustls::RootCertStore::empty();
    let result = rustls_native_certs::load_native_certs();
    for err in &result.errors {
        eprintln!("warning: failed to load a native cert: {err}");
    }
    let (added, _ignored) = store.add_parsable_certificates(result.certs);
    if added == 0 {
        bail!("no trusted root certificates found in the OS trust store");
    }
    Ok(store)
}

/// Accept any server certificate. Only reachable behind the explicit `--insecure`
/// flag, for a relay presenting a self-signed cert in local development.
#[derive(Debug)]
struct AcceptAnyCert;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_root_store_is_populated() {
        // Regression guard for the TLS-verification fix: the default (non-insecure)
        // path must build a non-empty trust store rather than accepting any cert.
        let store = native_root_store().expect("OS trust store should load");
        assert!(store.len() > 0, "expected at least one trusted root");
    }
}

fn print_targets(targets: &[SshTarget]) {
    println!(
        "{:<14} {:<24} {:<24} {:<16} {}",
        "INSTANCE", "AGENT", "HOSTNAME", "CLUSTER", "SSH PORT"
    );
    for t in targets {
        let port = t
            .ssh_port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".into());
        println!(
            "{:<14} {:<24} {:<24} {:<16} {}",
            &t.instance_id[..12.min(t.instance_id.len())],
            t.agent_name.as_deref().unwrap_or("-"),
            t.hostname.as_deref().unwrap_or("-"),
            t.cluster_name.as_deref().unwrap_or("-"),
            port,
        );
    }
}
