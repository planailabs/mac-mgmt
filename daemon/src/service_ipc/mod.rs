pub mod client;
pub mod listener;
pub mod protocol;

use std::path::PathBuf;

/// Well-known directory for per-service Unix sockets.
pub fn sockets_dir() -> PathBuf {
    crate::config::config_dir().join("sockets")
}

/// Socket path for a given service name.
pub fn socket_path(service_name: &str) -> PathBuf {
    sockets_dir().join(format!("{service_name}.sock"))
}
