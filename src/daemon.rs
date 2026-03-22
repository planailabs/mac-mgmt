use anyhow::Result;
use std::time::Duration;
use tokio::time;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour
const REPO_OWNER: &str = "plan-ai";
const REPO_NAME: &str = "mac-mgmt";
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
    tracing::info!("checking for updates...");

    match self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .current_version(self_update::cargo_crate_version!())
        .show_output(false)
        .no_confirm(true)
        .build()
        .and_then(|updater| updater.update())
    {
        Ok(status) => {
            if status.updated() {
                tracing::info!("updated to version {}", status.version());
            } else {
                tracing::info!("already up to date ({})", status.version());
            }
        }
        Err(e) => {
            tracing::warn!("update check failed: {e}");
        }
    }
}
