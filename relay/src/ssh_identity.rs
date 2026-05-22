//! Ephemeral SSH identity for the relay.
//!
//! The relay generates an in-memory Ed25519 SSH keypair on startup.
//! Daemons receive the public key during registration and add it to
//! their authorized keys, allowing the relay to open SSH sessions on
//! behalf of authenticated users (e.g. via client certificate auth).
//! The key is rotated on every relay restart.

use russh::keys::PrivateKey;

pub struct RelaySshIdentity {
    pub private_key: PrivateKey,
    /// OpenSSH-format public key string, e.g. `ssh-ed25519 AAAA... relay-ephemeral`.
    pub public_key_openssh: String,
}

impl RelaySshIdentity {
    /// Generate a fresh in-memory Ed25519 SSH keypair.
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        rand::Fill::fill(&mut seed, &mut rand::rng());
        let private_key = PrivateKey::new(
            russh::keys::ssh_key::private::KeypairData::Ed25519(
                russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&seed),
            ),
            "relay-ephemeral",
        )
        .expect("failed to create Ed25519 key");

        let public_key_openssh = {
            use russh::keys::PublicKeyBase64;
            let algo = private_key.algorithm();
            let b64 = private_key.public_key().public_key_base64();
            format!("{algo} {b64} relay-ephemeral")
        };

        tracing::info!("generated ephemeral relay SSH identity");
        Self {
            private_key,
            public_key_openssh,
        }
    }
}
