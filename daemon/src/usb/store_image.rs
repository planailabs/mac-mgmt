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

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Default relative path of the ext4 store image on the stick.
pub const IMAGE_REL_PATH: &str = "nix-store.img";

/// Default relative path of the on-stick `.nar` file binary cache.
pub const NAR_CACHE_REL_PATH: &str = "nix-cache";

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

/// Create the ext4 store image at the default size if it does not yet exist.
/// `size_bytes` is the initial size; ext4 images can be grown later.
pub fn ensure_image(_home: &Path, _size_bytes: u64) -> Result<PathBuf> {
    anyhow::bail!("ext4 store image creation not yet implemented")
}

/// Enter a private mount namespace and loop-mount the store image at `/nix`.
/// Must run as root. After this returns the process (and its children) see the
/// image-backed `/nix`, invisible to the host.
pub fn enter_namespace_and_mount(_home: &Path) -> Result<()> {
    anyhow::bail!("private mount namespace + /nix mount not yet implemented")
}
