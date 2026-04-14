//! Dynamic per-heartbeat sample. Kept cheap — target <50ms.
//!
//! Populated in step 6. For now returns an empty sample so the scaffolding
//! compiles and the wire format is exercised end-to-end.

use anyhow::Result;

use mac_mgmt_common::DynamicSample;

pub async fn collect() -> Result<DynamicSample> {
    Ok(DynamicSample::default())
}
