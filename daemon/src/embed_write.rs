//! Utilities for writing embedded assets to disk idempotently.

use anyhow::{Context, Result};
use std::path::Path;

/// Write an embedded file to disk if it doesn't exist or content differs.
/// Returns `true` if the file was already up-to-date.
pub fn materialize_embedded(content: &[u8], dest: &Path) -> Result<bool> {
    // Idempotency: skip if content matches.
    if dest.exists() {
        if let Ok(existing) = std::fs::read(dest) {
            if existing == content {
                return Ok(true);
            }
        }
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    std::fs::write(dest, content)
        .with_context(|| format!("failed to write {}", dest.display()))?;

    Ok(false)
}

/// Write all files from a rust_embed asset collection to a target directory.
/// Each file path in the embed is preserved relative to target_dir.
/// Returns the number of files that were written (not skipped).
pub fn materialize_all<E: rust_embed::Embed>(target_dir: &Path) -> Result<usize> {
    let mut written = 0;
    for path in E::iter() {
        if let Some(file) = E::get(&path) {
            let dest = target_dir.join(path.as_ref());
            let up_to_date = materialize_embedded(&file.data, &dest)?;
            if !up_to_date {
                tracing::debug!("materialize: wrote {} ({} bytes)", dest.display(), file.data.len());
                written += 1;
            }
        }
    }
    Ok(written)
}
