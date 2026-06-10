use anyhow::{Context, Result};
use std::process::{Command, Output};
use std::time::Duration;

/// Default timeout for commands run during the health/connector tick.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Run a command with a wall-clock timeout. If the process doesn't exit
/// within `timeout`, it is killed with SIGKILL.
pub fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<Output> {
    let child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("failed to spawn command")?;

    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => result.context("command execution failed"),
        Err(_) => {
            // Kill the process on timeout. The child was moved into the waiter
            // thread, so we signal by pid (Unix); on non-Unix the spawned waiter
            // owns the only handle and we simply report the timeout.
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
            #[cfg(not(unix))]
            let _ = pid;
            anyhow::bail!("command timed out after {timeout:?}");
        }
    }
}
