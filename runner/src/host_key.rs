//! Ed25519 host-key generation for pregenerated instance IDs.
//!
//! The daemon's `instance_id` is defined as `sha256(ssh_wire_pubkey)` (hex)
//! where the host key lives at `~/.config/mac-mgmt/host_ed25519_key`. To know
//! the daemon's instance_id before it boots, the runner generates the keypair,
//! computes the fingerprint, and embeds the PEM in the cloud-init so the
//! daemon loads it instead of generating its own.

use anyhow::{Context, Result};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use ssh_key::{Algorithm, LineEnding, PrivateKey};

/// A freshly-generated host key.
pub struct HostKey {
    /// OpenSSH PEM-encoded private key (exactly what the daemon's
    /// `host_keys::load_or_generate` writes).
    pub private_pem: String,
    /// Hex-encoded SHA-256 of the SSH-wire-format public key — matches
    /// the daemon's `fingerprint_hex(&key)`.
    pub instance_id: String,
}

pub fn generate() -> Result<HostKey> {
    let mut rng = OsRng;
    let key = PrivateKey::random(&mut rng, Algorithm::Ed25519)
        .context("generating ed25519 key")?;
    let pem = key
        .to_openssh(LineEnding::LF)
        .context("encoding openssh PEM")?
        .to_string();
    let pub_wire = key
        .public_key()
        .to_bytes()
        .context("encoding ssh-wire public key")?;
    let instance_id = hex::encode(Sha256::digest(&pub_wire));
    Ok(HostKey { private_pem: pem, instance_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Cross-check that the runner's fingerprint matches what the daemon
    // computes via `russh::keys::PublicKeyBase64::public_key_bytes()`.
    #[test]
    fn instance_id_matches_daemon_algorithm() {
        use russh::keys::{decode_secret_key, PublicKeyBase64};

        let hk = generate().expect("generate host key");
        let daemon_key = decode_secret_key(&hk.private_pem, None)
            .expect("daemon decodes our PEM");
        let daemon_wire = daemon_key.public_key_bytes();
        let daemon_fp = hex::encode(Sha256::digest(&daemon_wire));
        assert_eq!(
            hk.instance_id, daemon_fp,
            "runner instance_id must match daemon fingerprint"
        );
        assert_eq!(hk.instance_id.len(), 64, "SHA-256 hex is 64 chars");
    }
}
