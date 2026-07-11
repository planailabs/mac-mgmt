//! Small cross-platform shims for the handful of Unix syscalls the daemon
//! relies on, so the `usbd` control plane compiles (and runs, degraded where
//! the concept doesn't exist) on non-Unix targets such as Windows.
//!
//! The daemon was written Unix-first; rather than scatter `#[cfg(unix)]` over
//! every call site, the common cases (file mode bits, a SIGTERM-equivalent
//! shutdown signal, process re-exec) live here behind a portable surface.

use std::path::Path;

/// Whether the current process has root/superuser privileges. Always `false`
/// on non-Unix (the privilege model differs and the callers — sudo wrapping,
/// systemd unit management — are Unix-only).
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// The current real user id. `0` on non-Unix — only used to build
/// `/run/user/<uid>` paths for `systemctl --user`, which is Linux-only.
pub fn current_uid() -> u32 {
    #[cfg(unix)]
    {
        unsafe { libc::getuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// POSIX signal numbers used by the `systemctl` shim. On Unix these are the
/// libc values; elsewhere they are the standard Linux numeric values. The
/// systemctl/systemd surface only ever forwards these to a Linux host — they
/// are never raised on a non-Unix local process — so the Linux numbers are the
/// semantically correct fallback.
#[cfg(unix)]
pub use libc::{SIGCONT, SIGHUP, SIGINT, SIGKILL, SIGQUIT, SIGSTOP, SIGTERM, SIGUSR1, SIGUSR2};
#[cfg(not(unix))]
mod sig_consts {
    pub const SIGHUP: i32 = 1;
    pub const SIGINT: i32 = 2;
    pub const SIGQUIT: i32 = 3;
    pub const SIGKILL: i32 = 9;
    pub const SIGUSR1: i32 = 10;
    pub const SIGUSR2: i32 = 12;
    pub const SIGTERM: i32 = 15;
    pub const SIGCONT: i32 = 18;
    pub const SIGSTOP: i32 = 19;
}
#[cfg(not(unix))]
pub use sig_consts::*;

/// Set Unix permission bits on a path. No-op on non-Unix targets — Windows has
/// no `st_mode`; access is governed by ACLs we don't manage here, and the modes
/// we set (0o600 / 0o755) carry no meaning there.
pub fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// Replace the current process with `exe args`.
///
/// On Unix this is `execv` — it never returns on success, so the returned
/// `io::Error` is always the failure cause. On non-Unix there is no process
/// replacement, so we spawn the binary, wait for it, and exit with its status;
/// if the spawn itself fails we return that error (matching the Unix contract
/// that a returned error means the re-exec did not happen).
pub fn reexec(exe: &Path, args: &[String]) -> std::io::Error {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        std::process::Command::new(exe).args(args).exec()
    }
    #[cfg(not(unix))]
    {
        match std::process::Command::new(exe).args(args).status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(0)),
            Err(e) => e,
        }
    }
}

/// A shutdown signal source. On Unix it wraps the real SIGTERM/SIGINT stream;
/// on other targets it falls back to Ctrl-C (the only console signal tokio
/// exposes portably), so a `select!` on `recv()` behaves the same everywhere.
pub struct ShutdownSignal {
    #[cfg(unix)]
    inner: tokio::signal::unix::Signal,
}

impl ShutdownSignal {
    /// Register a SIGTERM-equivalent handler.
    pub fn terminate() -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            let inner = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            Ok(Self { inner })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    /// Register a SIGINT-equivalent handler.
    pub fn interrupt() -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            let inner = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
            Ok(Self { inner })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    /// Wait for the next signal.
    pub async fn recv(&mut self) {
        #[cfg(unix)]
        {
            self.inner.recv().await;
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}
