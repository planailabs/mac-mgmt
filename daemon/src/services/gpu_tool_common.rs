//! Shared helpers for GPU-tool install services (nvidia-smi, rocm-smi).

use std::io;
use std::process::Command;

/// `true` when `bin` resolves to an executable file on the current `$PATH`.
/// Portable across Linux/macOS — walks `$PATH` rather than shelling out to
/// `which`.
pub fn is_in_path(bin: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return true;
        }
    }
    false
}

/// Check whether `lspci` reports any display controller (class 0x0300) with
/// the given PCI vendor ID. Used to gate installs so we don't pull a
/// multi-GB toolkit onto GPU-less hosts.
///
/// Returns `Err` only when `lspci` itself is missing or failed; an empty
/// result (no matching device) returns `Ok(false)` so callers can
/// distinguish "no hardware" from "can't tell".
pub fn lspci_has_vendor(vendor_id_hex: &str) -> io::Result<bool> {
    let output = Command::new("lspci").args(["-mmn", "-d", "::0300"]).output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("lspci exited {:?}", output.status.code()),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let needle = format!("\"{}\"", vendor_id_hex.to_lowercase());
    Ok(stdout
        .lines()
        .any(|line| line.to_lowercase().contains(&needle)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_in_path_finds_ls() {
        // `ls` exists on every supported platform.
        assert!(is_in_path("ls"));
    }

    #[test]
    fn is_in_path_missing_binary() {
        assert!(!is_in_path("this-binary-definitely-does-not-exist-xyz"));
    }
}
