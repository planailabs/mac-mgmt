use anyhow::{Context, Result};
use std::io::Write;
use std::time::Duration;
use tokio::time;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour
const UPDATE_URL: &str = "https://update.plan.ai/mac-mgmt.tar.gz";
const BIN_NAME: &str = "mac-mgmt";

pub async fn run() -> Result<()> {
    tracing::info!("daemon started, checking for updates every {:?}", UPDATE_INTERVAL);

    let mut interval = time::interval(UPDATE_INTERVAL);

    // Run an immediate update check on startup
    check_and_update();

    loop {
        interval.tick().await;
        check_and_update();
    }
}

fn check_and_update() {
    tracing::info!("checking for updates from {UPDATE_URL}");

    if let Err(e) = do_update() {
        tracing::warn!("update failed: {e}");
    }
}

fn do_update() -> Result<()> {
    let mut tmp_archive = tempfile::Builder::new()
        .suffix(".tar.gz")
        .tempfile()
        .context("failed to create temp file")?;

    // Download the tarball
    tracing::info!("downloading {UPDATE_URL}");
    let mut download = self_update::Download::from_url(UPDATE_URL);
    download.show_progress(false);
    let mut body = Vec::new();
    download.download_to(&mut body)?;
    tmp_archive.write_all(&body)?;
    tmp_archive.flush()?;

    // Extract the binary from the archive
    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    self_update::Extract::from_source(tmp_archive.path())
        .archive(self_update::ArchiveKind::Tar(Some(
            self_update::Compression::Gz,
        )))
        .extract_into(tmp_dir.path())?;

    let new_bin = tmp_dir.path().join(BIN_NAME);
    if !new_bin.exists() {
        anyhow::bail!("binary '{BIN_NAME}' not found in archive");
    }

    // Replace the running binary
    let current_bin = std::env::current_exe().context("cannot determine current exe")?;
    self_update::Move::from_source(&new_bin)
        .replace_using_temp(&current_bin)
        .to_dest(&current_bin)?;

    tracing::info!("binary updated successfully");
    Ok(())
}
