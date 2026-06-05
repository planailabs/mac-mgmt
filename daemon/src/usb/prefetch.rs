//! Headless provisioning (`usb prefetch`).
//!
//! Downloads everything the stick needs so a subsequent `usb --offline` run
//! requires zero network: nix-portable, the pinned nixpkgs tarball, and a
//! mirror of every service closure (into the on-stick `.nar` cache). Shares its
//! download/cache helpers with the online first-run path. No webview — this is
//! an ordinary async command.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Options for the prefetch command.
#[derive(Clone, Debug)]
pub struct PrefetchOpts {
    /// Stick / `HOME` directory to provision.
    pub home: PathBuf,
    /// Config path (default `<home>/config.toml`).
    pub config_path: Option<PathBuf>,
    /// Download nix-portable for all supported arches, not just the host's, so
    /// one stick boots on multiple machines.
    pub all_arches: bool,
}

/// What a prefetch run obtained, for the printed manifest.
#[derive(Default, Debug)]
pub struct Manifest {
    pub nix_portable: Vec<String>,
    pub nixpkgs_rev: Option<String>,
    pub closures: Vec<String>,
    pub release_binary: Option<String>,
}

/// Run the headless prefetch. Online by definition; fails non-zero if any
/// required artifact cannot be obtained.
pub async fn run(opts: PrefetchOpts) -> Result<Manifest> {
    std::fs::create_dir_all(&opts.home)
        .with_context(|| format!("failed to create home dir {}", opts.home.display()))?;
    tracing::info!(home = %opts.home.display(), all_arches = opts.all_arches, "prefetch starting");

    anyhow::bail!("usb prefetch is not yet implemented")
}
