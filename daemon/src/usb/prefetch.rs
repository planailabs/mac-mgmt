//! Headless provisioning (`usb prefetch`).
//!
//! Downloads everything the stick needs so a subsequent `usb --offline` run
//! requires zero network: nix-portable, the pinned nixpkgs tarball, the ext4
//! store image, and a mirror of every requested package closure (into the
//! on-stick `.nar` cache). Shares its download/cache helpers with the online
//! first-run path. No webview — this is an ordinary async command.

use super::{nix_darwin, nix_portable, nixpkgs, runtime, store_image};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

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
    /// Pinned nixpkgs revision (defaults to [`nixpkgs::DEFAULT_REV`]).
    pub nixpkgs_rev: Option<String>,
    /// Package names (resolved against the pinned nixpkgs) whose closures to
    /// mirror into the on-stick `.nar` cache, e.g. `["ollama"]`.
    pub packages: Vec<String>,
}

/// What a prefetch run obtained, for the printed manifest.
#[derive(Default, Debug)]
pub struct Manifest {
    pub nix_portable: Vec<String>,
    pub nixpkgs_rev: Option<String>,
    pub closures: Vec<String>,
}

/// Run the headless prefetch. Online by definition; fails non-zero if any
/// required artifact cannot be obtained.
pub async fn run(opts: PrefetchOpts) -> Result<Manifest> {
    std::fs::create_dir_all(&opts.home)
        .with_context(|| format!("failed to create home dir {}", opts.home.display()))?;
    tracing::info!(home = %opts.home.display(), all_arches = opts.all_arches, "prefetch starting");

    let home = opts.home.clone();
    let rev = opts
        .nixpkgs_rev
        .clone()
        .unwrap_or_else(|| nixpkgs::DEFAULT_REV.to_string());
    let all_arches = opts.all_arches;

    // Blocking downloads + image creation off the async runtime.
    let (home2, rev2) = (home.clone(), rev.clone());
    let mut manifest: Manifest = tokio::task::spawn_blocking(move || -> Result<Manifest> {
        let mut m = Manifest::default();

        // 1. Runtime bootstrap: nix-portable on Linux, the native nix install
        //    tarball on macOS (nix-portable can't run on Darwin).
        if std::env::consts::OS == "macos" {
            if let Some(system) = nix_darwin::darwin_system() {
                let tb = home2.join(".cache/usb").join(format!(
                    "nix-{}-{}.tar.xz",
                    nix_darwin::PINNED_NIX_VERSION,
                    system
                ));
                if !tb.exists() {
                    nix_darwin::download_tarball(&tb, system).context("native nix tarball")?;
                }
                m.nix_portable.push(system.to_string());
            }
        } else if all_arches {
            for (arch, asset) in nix_portable::all_assets() {
                let dest = nix_portable::cached_path_for_arch(&home2, arch);
                if !dest.exists() {
                    // Reuse the verified-download path by writing to the per-arch slot.
                    download_asset_blocking(&asset, &dest)?;
                }
                m.nix_portable.push(arch.to_string());
            }
        } else {
            nix_portable::ensure_blocking(&home2, true).context("nix-portable")?;
            m.nix_portable.push(nix_portable::host_arch());
        }

        // 2. Pinned nixpkgs tarball.
        nixpkgs::ensure_blocking(&home2, &rev2, true).context("nixpkgs tarball")?;
        m.nixpkgs_rev = Some(rev2.clone());

        // 3. Store image (ext4 on Linux, APFS sparsebundle on macOS) + nar cache.
        if std::env::consts::OS == "macos" {
            store_image::ensure_image_macos(&home2, store_image::DEFAULT_IMAGE_SIZE)
                .context("APFS store image")?;
        } else {
            store_image::ensure_image(&home2, store_image::DEFAULT_IMAGE_SIZE)
                .context("store image")?;
        }
        store_image::ensure_nar_cache(&home2).context("nar cache")?;

        Ok(m)
    })
    .await
    .context("prefetch blocking stage panicked")??;

    // 4. Mirror each requested package closure into the on-stick cache. Needs a
    //    usable nix; uses the host nix if present, else the just-downloaded
    //    nix-portable, evaluating against the pinned nixpkgs tarball.
    if !opts.packages.is_empty() {
        let nixpkgs_tarball = nixpkgs::cached_path(&home, &rev);
        let base = nixpkgs::flakeref_base(&nixpkgs_tarball);
        let cache = store_image::nar_cache_path(&home);
        configure_prefetch_nix_env(&home, &base);
        for pkg in &opts.packages {
            mirror_closure(&base, &cache, pkg)
                .await
                .with_context(|| format!("failed to mirror closure for {pkg}"))?;
            manifest.closures.push(pkg.clone());
        }
    } else {
        tracing::info!("no packages requested; mirrored infra only (image, nix-portable, nixpkgs)");
    }

    Ok(manifest)
}

/// Configure the process env so `crate::nix` invocations during prefetch use
/// the host nix (if any) or nix-portable, evaluating against the pinned
/// nixpkgs tarball.
fn configure_prefetch_nix_env(home: &Path, nixpkgs_base: &str) {
    // SAFETY: prefetch runs as a standalone command; env is process-local.
    unsafe {
        std::env::set_var("MAC_MGMT_NIXPKGS_TARBALL", nixpkgs_base);
    }
    if runtime::detect_host_nix().is_none() {
        let np = nix_portable::cached_path(home);
        unsafe {
            std::env::set_var("MAC_MGMT_NIX_WRAPPER", &np);
            std::env::set_var("NP_LOCATION", home);
        }
    }
}

/// `nix copy --to file://<cache> <base>#<pkg>` — builds/substitutes the package
/// (from xzar / cache.nixos.org) and copies its closure into the on-stick cache.
async fn mirror_closure(base: &str, cache: &Path, pkg: &str) -> Result<()> {
    let installable = format!("{base}#{pkg}");
    let to = format!("file://{}", cache.display());
    tracing::info!("mirroring {installable} -> {to}");
    let cache_args = crate::nix::extra_substituter_args();
    let mut cmd = crate::nix::nix_command("nix");
    cmd.env("NIXPKGS_ALLOW_UNFREE", "1")
        .args(["copy", "--no-check-sigs", "--to", &to, &installable])
        .args(&cache_args);
    let pkg_owned = pkg.to_string();
    let out = tokio::task::spawn_blocking(move || cmd.output())
        .await
        .context("nix copy task panicked")?
        .with_context(|| format!("failed to run nix copy for {pkg_owned}"))?;
    if !out.status.success() {
        anyhow::bail!(
            "nix copy {installable} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Download a specific nix-portable asset to an explicit destination (used for
/// `--all-arches`, where each arch goes to its own per-arch cache slot).
fn download_asset_blocking(asset: &nix_portable::Asset, dest: &Path) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let url = nix_portable::asset_url(nix_portable::PINNED_TAG, asset);
    tracing::info!("downloading {url}");
    let bytes = reqwest::blocking::Client::new()
        .get(&url)
        .send()
        .with_context(|| format!("failed to GET {url}"))?
        .error_for_status()
        .with_context(|| format!("bad status from {url}"))?
        .bytes()
        .context("failed to read body")?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(asset.sha256) {
        anyhow::bail!("sha256 mismatch for {}: got {got}", asset.file_name);
    }
    let tmp = dest.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}
