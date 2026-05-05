//! web-agency-upload: upload a folder to a web-agency Cloudflare Pages webspace.
//!
//! Usage:
//!   web-agency-upload ./dist
//!   web-agency-upload --webspace-id UUID ./dist
//!   WEB_AGENCY_TOKEN=... WEB_AGENCY_URL=... web-agency-upload ./dist

use anyhow::{Context, Result, bail};
use clap::Parser;
use flate2::Compression;
use flate2::write::GzEncoder;
use serde::Deserialize;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "web-agency-upload", version, about = "Upload a site folder to a web-agency webspace")]
struct Cli {
    /// Directory to upload (will be tarred and gzipped)
    path: PathBuf,

    /// Deploy token (or set WEB_AGENCY_TOKEN env)
    #[arg(long, env = "WEB_AGENCY_TOKEN")]
    token: String,

    /// Server URL (or set WEB_AGENCY_URL env)
    #[arg(long, env = "WEB_AGENCY_URL")]
    url: String,

    /// Webspace ID to deploy to. If omitted, uses the token's scoped webspace.
    #[arg(long, env = "WEB_AGENCY_WEBSPACE_ID")]
    webspace_id: Option<String>,

    /// Branch name for the deployment (e.g. "main", "preview", "staging").
    /// Maps to a Cloudflare Pages deployment branch.
    #[arg(long, env = "WEB_AGENCY_BRANCH")]
    branch: Option<String>,

    /// Poll interval in seconds when waiting for deployment
    #[arg(long, default_value = "3")]
    poll_interval: u64,

    /// Don't wait for deployment to finish
    #[arg(long)]
    no_wait: bool,
}

#[derive(Deserialize)]
struct WhoamiResponse {
    kind: String,
    webspace_id: Option<String>,
    webspace_name: Option<String>,
}

#[derive(Deserialize)]
struct UploadResponse {
    deployment_id: String,
    status: String,
}

#[derive(Deserialize)]
struct StatusResponse {
    deployment_id: String,
    status: String,
    error_message: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let base_url = cli.url.trim_end_matches('/');
    let client = reqwest::blocking::Client::new();

    // Resolve webspace ID
    let webspace_id = match cli.webspace_id {
        Some(id) => id,
        None => {
            eprintln!("No --webspace-id specified, checking token scope...");
            let resp = client
                .get(format!("{base_url}/api/v1/deploy/whoami"))
                .bearer_auth(&cli.token)
                .send()
                .context("failed to reach server")?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().unwrap_or_default();
                bail!("whoami failed ({status}): {body}");
            }

            let whoami: WhoamiResponse = resp.json().context("invalid whoami response")?;

            if whoami.kind != "deploy" {
                bail!("token kind is '{}', expected 'deploy'", whoami.kind);
            }

            match whoami.webspace_id {
                Some(id) => {
                    let name = whoami.webspace_name.as_deref().unwrap_or("unknown");
                    eprintln!("Using token's scoped webspace: {name} ({id})");
                    id
                }
                None => bail!(
                    "token is not scoped to a specific webspace. \
                     Use --webspace-id or WEB_AGENCY_WEBSPACE_ID to specify one."
                ),
            }
        }
    };

    // Validate path
    if !cli.path.is_dir() {
        bail!("{} is not a directory", cli.path.display());
    }

    // Create tarball in memory
    eprintln!("Packaging {}...", cli.path.display());
    let tarball = create_tarball(&cli.path)?;
    eprintln!("Tarball size: {} bytes ({:.1} KB)", tarball.len(), tarball.len() as f64 / 1024.0);

    // Upload
    let mut upload_url = format!("{base_url}/api/v1/deploy/{webspace_id}");
    if let Some(ref branch) = cli.branch {
        upload_url = format!("{upload_url}?branch={}", utf8_percent_encode(branch, NON_ALPHANUMERIC));
        eprintln!("Uploading to webspace {webspace_id} (branch: {branch})...");
    } else {
        eprintln!("Uploading to webspace {webspace_id}...");
    }
    let resp = client
        .post(&upload_url)
        .bearer_auth(&cli.token)
        .body(tarball)
        .send()
        .context("upload failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        bail!("upload failed ({status}): {body}");
    }

    let upload: UploadResponse = resp.json().context("invalid upload response")?;
    eprintln!("Deployment {} started (status: {})", upload.deployment_id, upload.status);

    if cli.no_wait {
        println!("{}", serde_json::json!({
            "deployment_id": upload.deployment_id,
            "status": upload.status,
        }));
        return Ok(());
    }

    // Poll for completion
    eprintln!("Waiting for deployment...");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(cli.poll_interval));

        let resp = client
            .get(format!("{base_url}/api/v1/deploy/{webspace_id}/status"))
            .bearer_auth(&cli.token)
            .send()
            .context("status check failed")?;

        if !resp.status().is_success() {
            let body = resp.text().unwrap_or_default();
            eprintln!("Warning: status check failed: {body}");
            continue;
        }

        let status: StatusResponse = resp.json().context("invalid status response")?;
        eprint!("\rStatus: {:<20}", status.status);

        match status.status.as_str() {
            "success" => {
                eprintln!("\nDeployment successful!");
                println!("{}", serde_json::json!({
                    "deployment_id": status.deployment_id,
                    "status": "success",
                }));
                return Ok(());
            }
            "failed" => {
                let msg = status.error_message.as_deref().unwrap_or("unknown error");
                eprintln!("\nDeployment failed: {msg}");
                std::process::exit(1);
            }
            _ => {} // keep polling
        }
    }
}

fn create_tarball(dir: &std::path::Path) -> Result<Vec<u8>> {
    let buf = Vec::new();
    let enc = GzEncoder::new(buf, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(".", dir)
        .context("failed to add directory to tarball")?;
    let enc = tar.into_inner().context("failed to finish tarball")?;
    let buf = enc.finish().context("failed to finish gzip")?;
    Ok(buf)
}
