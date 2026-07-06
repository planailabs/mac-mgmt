//! Ed25519 host-key generation → pre-computed daemon instance_id.
//! Ported from `runner/src/host_key.rs`: instance_id = hex(sha256(ssh wire pubkey)),
//! which matches the daemon's `fingerprint_hex`.

use anyhow::{Context, Result};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use ssh_key::{Algorithm, LineEnding, PrivateKey};

pub struct HostKey {
    pub private_pem: String,
    pub instance_id: String,
}

pub fn generate() -> Result<HostKey> {
    let mut rng = OsRng;
    let key = PrivateKey::random(&mut rng, Algorithm::Ed25519).context("generating ed25519 key")?;
    let private_pem = key
        .to_openssh(LineEnding::LF)
        .context("encoding openssh PEM")?
        .to_string();
    let pub_wire = key
        .public_key()
        .to_bytes()
        .context("encoding ssh-wire public key")?;
    let instance_id = hex::encode(Sha256::digest(&pub_wire));
    Ok(HostKey {
        private_pem,
        instance_id,
    })
}
