//! Security posture: SIP/FileVault/firewall/Gatekeeper on macOS; SELinux/AppArmor/
//! ufw/nftables/FDE on Linux. Booleans + version strings only — no secrets.
//!
//! Populated in step 5.

use anyhow::Result;

use mac_mgmt_common::SecurityPosture;

pub async fn collect() -> Result<SecurityPosture> {
    Ok(SecurityPosture::default())
}
