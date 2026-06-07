//! Cross-platform local-socket transport for the daemon<->supervisor RPC.
//!
//! Unix domain socket (a filesystem path) on unix; a namespaced named pipe on
//! windows. The daemon and supervisor both derive the endpoint from the same
//! agreed `socket_path`, so they always meet at the same name.

use anyhow::{Context, Result};
use std::path::Path;

use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::{Listener, Stream};
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;

/// The per-connection stream (tokio AsyncRead + AsyncWrite).
pub type IpcStream = Stream;

/// Bind the listening endpoint (supervisor side).
pub fn bind(socket_path: &Path) -> Result<Listener> {
    #[cfg(unix)]
    {
        // Clear a stale socket file from a previous run.
        if socket_path.exists() {
            std::fs::remove_file(socket_path).ok();
        }
        let name = socket_path
            .to_fs_name::<GenericFilePath>()
            .context("local-socket fs name")?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .with_context(|| format!("bind local socket {}", socket_path.display()))
    }
    #[cfg(windows)]
    {
        let stem = pipe_stem(socket_path);
        let name = stem
            .to_ns_name::<GenericNamespaced>()
            .context("named-pipe ns name")?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .with_context(|| format!("bind named pipe {stem}"))
    }
}

/// Accept one inbound connection (supervisor side).
pub async fn accept(listener: &Listener) -> std::io::Result<Stream> {
    use interprocess::local_socket::traits::tokio::Listener as _;
    listener.accept().await
}

/// Connect to the supervisor (daemon side).
pub async fn connect(socket_path: &Path) -> std::io::Result<Stream> {
    #[cfg(unix)]
    {
        let name = socket_path.to_fs_name::<GenericFilePath>().map_err(to_io)?;
        Stream::connect(name).await
    }
    #[cfg(windows)]
    {
        let stem = pipe_stem(socket_path);
        let name = stem.to_ns_name::<GenericNamespaced>().map_err(to_io)?;
        Stream::connect(name).await
    }
}

#[cfg(windows)]
fn pipe_stem(socket_path: &Path) -> String {
    socket_path
        .file_name()
        .map(|s| s.to_string_lossy().replace('.', "-"))
        .unwrap_or_else(|| "mac-mgmt-services".into())
}

#[cfg(windows)]
fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
}

#[cfg(unix)]
fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
}
