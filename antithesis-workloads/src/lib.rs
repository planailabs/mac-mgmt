//! Antithesis-SDK-wired workloads that drive a mac-mgmt cluster and assert
//! eventual-consistency properties.
//!
//! The same workloads run locally under the mmrc emulator (against a fresh
//! incus cluster) and on the real Antithesis platform. Environment-specific
//! behaviour (base URLs, tokens, how nodes are spawned) is hidden behind the
//! [`env::Env`] trait, implemented in `mmr-causality`.

pub mod assert;
pub mod env;
pub mod goals;
pub mod mgmt;
pub mod relay;
pub mod rng;
pub mod workloads;

pub use env::{Ctx, Env, NodeKind, Timeouts};
pub use mgmt::MgmtApi;
pub use relay::{HostMode, RelayApi};
pub use rng::Rng;
pub use workloads::{Outcome, Workload, registry};

/// Initialise the Antithesis SDK (and default local JSONL output path).
pub fn init(local_output: Option<&std::path::Path>) {
    assert::init(local_output);
}
