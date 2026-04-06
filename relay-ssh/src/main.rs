use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "relay-ssh", version, about = "SSH into mac-mgmt managed machines via relay")]
struct Cli {
    /// Instance ID or prefix to connect to
    instance_id: Option<String>,

    /// List active tunnels
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
}

#[derive(Deserialize)]
struct Config {
    relay_url: Option<String>,
    token: Option<String>,
}

#[derive(Deserialize)]
struct TunnelInfo {
    instance_id: String,
    customer_name: Option<String>,
    agent_name: Option<String>,
    ssh_port: u16,
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
    let cli = Cli::parse();
    let config = load_config();

    let relay_url = cli
        .relay
        .or_else(|| std::env::var("RELAY_URL").ok())
        .or(config.relay_url)
        .context("relay URL not configured (use --relay, RELAY_URL env, or config file)")?;

    let token = cli
        .token
        .or_else(|| std::env::var("RELAY_TOKEN").ok())
        .or(config.token)
        .context("token not configured (use --token, RELAY_TOKEN env, or config file)")?;

    let tunnels = fetch_tunnels(&relay_url, &token).await?;

    if cli.list {
        print_tunnels(&tunnels);
        return Ok(());
    }

    let tunnel = match &cli.instance_id {
        Some(prefix) => {
            let matches: Vec<_> = tunnels
                .iter()
                .filter(|t| t.instance_id.starts_with(prefix.as_str()))
                .collect();
            match matches.len() {
                0 => bail!("no tunnel matching prefix '{prefix}'"),
                1 => matches[0],
                n => {
                    eprintln!("ambiguous prefix '{prefix}', matches {n} tunnels:");
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
            if tunnels.len() == 1 {
                &tunnels[0]
            } else if tunnels.is_empty() {
                bail!("no active tunnels");
            } else {
                eprintln!("multiple tunnels active, specify an instance ID:");
                print_tunnels(&tunnels);
                bail!("specify an instance ID or prefix");
            }
        }
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
        tunnel.agent_name.as_deref().unwrap_or("-"),
        &tunnel.instance_id[..12.min(tunnel.instance_id.len())],
        host,
        tunnel.ssh_port,
    );

    let mut cmd = Command::new("ssh");
    cmd.arg("-p").arg(tunnel.ssh_port.to_string());

    if let Some(user) = &cli.user {
        cmd.arg(format!("{user}@{host}"));
    } else {
        cmd.arg(host);
    }

    let status = cmd.status().context("failed to exec ssh")?;
    std::process::exit(status.code().unwrap_or(1));
}

async fn fetch_tunnels(relay_url: &str, token: &str) -> Result<Vec<TunnelInfo>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{relay_url}/api/tunnels"))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach relay")?;

    if !resp.status().is_success() {
        bail!("relay returned {}", resp.status());
    }

    resp.json().await.context("failed to parse tunnel list")
}

fn print_tunnels(tunnels: &[TunnelInfo]) {
    println!(
        "{:<14} {:<24} {:<16} {}",
        "INSTANCE", "AGENT", "CUSTOMER", "PORT"
    );
    for t in tunnels {
        println!(
            "{:<14} {:<24} {:<16} {}",
            &t.instance_id[..12.min(t.instance_id.len())],
            t.agent_name.as_deref().unwrap_or("-"),
            t.customer_name.as_deref().unwrap_or("-"),
            t.ssh_port,
        );
    }
}
