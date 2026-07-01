//! Endpoint modules. Each holds the shared handlers plus their macro-generated
//! Dioxus `#[server]` wrappers for one entity.

pub mod certificates;
pub mod contacts;
pub mod credentials;
pub mod domains;
pub mod organizations;
pub mod webspace_hosts;
pub mod webspaces;
