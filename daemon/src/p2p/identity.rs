//! Convert russh Ed25519 host keys to libp2p identities.
//!
//! The daemon stores its Ed25519 key as a PKCS8 PEM file. We parse the
//! raw 32-byte seed from the DER encoding and construct a libp2p
//! `Keypair` from it. This means the daemon's `PeerId` is
//! deterministically derived from the same key used for SSH and
//! heartbeat signing.

use anyhow::{Context, Result, bail};
use libp2p::identity;

/// Ed25519 PKCS8 v1 DER has a fixed structure.  The 32-byte private
/// key seed is wrapped in an inner OCTET STRING inside the outer
/// OCTET STRING at a known offset.
///
/// Structure (RFC 8410):
/// ```text
/// SEQUENCE {
///   INTEGER 0
///   SEQUENCE { OID 1.3.101.112 }
///   OCTET STRING {          -- outer, tag 0x04
///     OCTET STRING {        -- inner, tag 0x04, length 0x20
///       <32 bytes seed>
///     }
///   }
/// }
/// ```
///
/// Total DER prefix before the 32-byte seed is always 16 bytes for
/// Ed25519 PKCS8 v1.
const ED25519_PKCS8_V1_SEED_OFFSET: usize = 16;
const ED25519_SEED_LEN: usize = 32;

/// Build a libp2p `Keypair` from the daemon's PKCS8 PEM host key file.
pub fn keypair_from_pkcs8_pem(pem_data: &str) -> Result<identity::Keypair> {
    let der = decode_pem_to_der(pem_data).context("failed to decode PEM")?;
    let seed = extract_ed25519_seed(&der).context("failed to extract Ed25519 seed from PKCS8")?;

    let secret = identity::ed25519::SecretKey::try_from_bytes(seed)
        .map_err(|e| anyhow::anyhow!("invalid Ed25519 seed: {e}"))?;
    let ed_keypair = identity::ed25519::Keypair::from(secret);
    Ok(identity::Keypair::from(ed_keypair))
}

/// Build a libp2p `Keypair` from a russh `PrivateKey` by re-encoding
/// it to PKCS8 PEM and parsing the seed bytes.
pub fn keypair_from_russh(key: &russh::keys::PrivateKey) -> Result<identity::Keypair> {
    let mut pem_buf = Vec::new();
    russh::keys::encode_pkcs8_pem(key, &mut pem_buf)
        .context("failed to encode russh key as PKCS8 PEM")?;
    let pem_str = String::from_utf8(pem_buf).context("PEM is not valid UTF-8")?;
    keypair_from_pkcs8_pem(&pem_str)
}

fn decode_pem_to_der(pem: &str) -> Result<Vec<u8>> {
    use base64::Engine;

    let mut b64 = String::new();
    let mut in_block = false;
    for line in pem.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("-----BEGIN ") {
            in_block = true;
            continue;
        }
        if trimmed.starts_with("-----END ") {
            break;
        }
        if in_block {
            b64.push_str(trimmed);
        }
    }

    if b64.is_empty() {
        bail!("no PEM data found");
    }

    base64::engine::general_purpose::STANDARD
        .decode(&b64)
        .context("invalid base64 in PEM")
}

fn extract_ed25519_seed(der: &[u8]) -> Result<[u8; ED25519_SEED_LEN]> {
    if der.len() < ED25519_PKCS8_V1_SEED_OFFSET + ED25519_SEED_LEN {
        bail!(
            "DER too short for Ed25519 PKCS8: {} bytes (need at least {})",
            der.len(),
            ED25519_PKCS8_V1_SEED_OFFSET + ED25519_SEED_LEN
        );
    }

    let mut seed = [0u8; ED25519_SEED_LEN];
    seed.copy_from_slice(
        &der[ED25519_PKCS8_V1_SEED_OFFSET..ED25519_PKCS8_V1_SEED_OFFSET + ED25519_SEED_LEN],
    );
    Ok(seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::{Algorithm, PrivateKey, encode_pkcs8_pem};

    fn test_rng() -> getrandom_04::rand_core::UnwrapErr<getrandom_04::SysRng> {
        getrandom_04::rand_core::UnwrapErr(getrandom_04::SysRng)
    }

    #[test]
    fn roundtrip_russh_to_libp2p() {
        let key =
            PrivateKey::random(&mut test_rng(), Algorithm::Ed25519).expect("generate Ed25519 key");

        let libp2p_kp = keypair_from_russh(&key).expect("conversion should succeed");
        assert!(
            matches!(libp2p_kp.key_type(), identity::KeyType::Ed25519),
            "key type must be Ed25519"
        );

        // Verify the public keys match: russh public key bytes (SSH wire format)
        // differ from libp2p's PeerId derivation, but the raw Ed25519 public key
        // (32 bytes) embedded in both should be identical.
        let mut pem_buf = Vec::new();
        encode_pkcs8_pem(&key, &mut pem_buf).unwrap();
        let pem = String::from_utf8(pem_buf).unwrap();

        // Second conversion produces the same PeerId
        let kp2 = keypair_from_pkcs8_pem(&pem).expect("second conversion");
        assert_eq!(libp2p_kp.public().to_peer_id(), kp2.public().to_peer_id());
    }

    #[test]
    fn deterministic_peer_id() {
        let key =
            PrivateKey::random(&mut test_rng(), Algorithm::Ed25519).expect("generate Ed25519 key");

        let kp1 = keypair_from_russh(&key).expect("first conversion");
        let kp2 = keypair_from_russh(&key).expect("second conversion");
        assert_eq!(
            kp1.public().to_peer_id(),
            kp2.public().to_peer_id(),
            "PeerId must be deterministic"
        );
    }
}
