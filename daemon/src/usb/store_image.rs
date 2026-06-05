//! On-stick nix store image + private mount namespace (Linux).
//!
//! A live nix store needs symlinks, POSIX perms, hardlinks and >4 GB files —
//! FAT32 has none of these. So the drive holds a real-FS **image file** (ext4
//! on Linux) which we loop-mount at `/nix` **inside a private mount namespace**,
//! keeping the standard `/nix/store` prefix (→ full binary-cache reuse, no
//! rebuild) and never disturbing a host's own `/nix/store`.
//!
//! The on-stick `.nar` file cache is a separate, plain-files binary cache used
//! as the offline substituter into this image.
//!
//! The namespace + mount steps require root and must run **before** the tokio
//! runtime is built (see [`crate::usb`]): a mount-namespace unshare moves only
//! the calling thread, so we do it single-threaded so every later worker thread
//! and child process inherits the image-backed `/nix`.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Default relative path of the ext4 store image on the stick.
pub const IMAGE_REL_PATH: &str = "nix-store.img";

/// Default relative path of the on-stick `.nar` file binary cache.
pub const NAR_CACHE_REL_PATH: &str = "nix-cache";

/// Default initial (sparse) size of the store image: 16 GiB. ext4 in a sparse
/// file only consumes the blocks actually written.
pub const DEFAULT_IMAGE_SIZE: u64 = 16 * 1024 * 1024 * 1024;

/// Absolute path of the store image under `home`.
pub fn image_path(home: &Path) -> PathBuf {
    home.join(IMAGE_REL_PATH)
}

/// Absolute path of the `.nar` file cache under `home`.
pub fn nar_cache_path(home: &Path) -> PathBuf {
    home.join(NAR_CACHE_REL_PATH)
}

/// `file://` substituter URL for the on-stick `.nar` cache.
pub fn nar_cache_substituter(home: &Path) -> String {
    format!("file://{}", nar_cache_path(home).display())
}

/// Create the ext4 store image at `size_bytes` (sparse) if it does not yet
/// exist. Returns the image path. Requires `mkfs.ext4` on PATH.
pub fn ensure_image(home: &Path, size_bytes: u64) -> Result<PathBuf> {
    let img = image_path(home);
    if img.exists() {
        return Ok(img);
    }
    if let Some(parent) = img.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    // Sparse-allocate the backing file.
    let f = std::fs::File::create(&img)
        .with_context(|| format!("failed to create image {}", img.display()))?;
    f.set_len(size_bytes)
        .with_context(|| format!("failed to size image {}", img.display()))?;
    drop(f);

    let out = Command::new("mkfs.ext4")
        .args(["-F", "-q", "-L", "nix-store"])
        .arg(&img)
        .output()
        .context("failed to run mkfs.ext4 (is e2fsprogs installed?)")?;
    if !out.status.success() {
        // Don't leave a half-formatted image behind.
        let _ = std::fs::remove_file(&img);
        anyhow::bail!(
            "mkfs.ext4 failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    tracing::info!("created ext4 store image at {}", img.display());
    Ok(img)
}

/// Initialise the on-stick `.nar` file binary cache directory (a valid empty
/// `nix-cache-info`), so nix accepts `file://…` as a substituter offline.
pub fn ensure_nar_cache(home: &Path) -> Result<PathBuf> {
    let dir = nar_cache_path(home);
    std::fs::create_dir_all(dir.join("nar"))
        .with_context(|| format!("failed to create {}", dir.display()))?;
    let info = dir.join("nix-cache-info");
    if !info.exists() {
        std::fs::write(&info, "StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 10\n")
            .with_context(|| format!("failed to write {}", info.display()))?;
    }
    Ok(dir)
}

/// Enter a private mount namespace and loop-mount the store image at `/nix`.
///
/// Must run as root and **single-threaded** (before the tokio runtime). After
/// this returns, this process and every thread/child it later spawns see the
/// image-backed `/nix`, invisible to the host.
pub fn enter_namespace_and_mount(home: &Path) -> Result<()> {
    let img = image_path(home);
    if !img.exists() {
        anyhow::bail!(
            "store image {} does not exist — run `usb prefetch` first",
            img.display()
        );
    }

    // New mount namespace for this process (CAP_SYS_ADMIN / root required).
    if unsafe { libc::unshare(libc::CLONE_NEWNS) } != 0 {
        let err = std::io::Error::last_os_error();
        anyhow::bail!(
            "unshare(CLONE_NEWNS) failed: {err} — the usb stack needs root (or CAP_SYS_ADMIN) \
             to mount its private /nix"
        );
    }

    // Make all mounts private so nothing we do propagates back to the host's
    // mount namespace (and the host's /nix, if any, stays untouched).
    make_rprivate().context("failed to make mounts private in new namespace")?;

    // /nix must exist as a mountpoint. On most hosts it already does; create it
    // best-effort otherwise.
    let _ = std::fs::create_dir_all("/nix");

    let st = Command::new("mount")
        .args(["-o", "loop"])
        .arg(&img)
        .arg("/nix")
        .status()
        .context("failed to spawn mount")?;
    if !st.success() {
        anyhow::bail!("failed to loop-mount {} at /nix", img.display());
    }
    tracing::info!("mounted store image at /nix in private namespace");
    Ok(())
}

/// `mount(NULL, "/", NULL, MS_REC|MS_PRIVATE, NULL)` — stop mount propagation.
fn make_rprivate() -> Result<()> {
    let root = std::ffi::CString::new("/").unwrap();
    let rc = unsafe {
        libc::mount(
            std::ptr::null(),
            root.as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("mount --make-rprivate /");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_under_home() {
        let home = Path::new("/stick/home");
        assert_eq!(image_path(home), Path::new("/stick/home/nix-store.img"));
        assert_eq!(nar_cache_path(home), Path::new("/stick/home/nix-cache"));
        assert_eq!(
            nar_cache_substituter(home),
            "file:///stick/home/nix-cache".to_string()
        );
    }

    #[test]
    fn ensure_nar_cache_writes_info() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ensure_nar_cache(tmp.path()).unwrap();
        let info = std::fs::read_to_string(dir.join("nix-cache-info")).unwrap();
        assert!(info.contains("StoreDir: /nix/store"));
        assert!(dir.join("nar").is_dir());
    }
}
