//! Static inventory collection: OS, kernel, CPU, RAM, disks, interfaces, nix.
//!
//! Populated in step 5 with platform-specific probes behind cfg gates.

use anyhow::Result;

use mac_mgmt_common::Inventory;

pub async fn collect() -> Result<Inventory> {
    Ok(Inventory::default())
}
