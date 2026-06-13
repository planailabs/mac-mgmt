use anyhow::{Context, Result};
use std::ffi::CString;
use std::io::Read;
use std::os::unix::io::FromRawFd;
use std::path::PathBuf;
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;

use super::RemoteSshCommand;

fn fifo_path() -> PathBuf {
    // Runtime dir, NOT config_dir: the config dir can be on FAT32 (the
    // plan-ai-usb stick) where mkfifo fails with EPERM. See config::runtime_dir.
    crate::config::runtime_dir().join("remote-ssh")
}

fn create_fifo(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).ok();
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let c_path =
        CString::new(path.to_str().context("non-UTF8 path")?).context("path contains null byte")?;
    let ret = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("mkfifo failed for {}", path.display()));
    }
    Ok(())
}

/// Remove the FIFO file. Call during daemon shutdown.
pub fn cleanup() {
    let path = fifo_path();
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!("failed to remove FIFO {}: {e}", path.display());
        } else {
            tracing::debug!("removed FIFO {}", path.display());
        }
    }
}

pub async fn watch(tx: mpsc::Sender<RemoteSshCommand>) -> Result<()> {
    let path = fifo_path();
    create_fifo(&path)?;
    tracing::info!("FIFO created at {}", path.display());

    // Open with O_RDWR | O_NONBLOCK so open() doesn't block waiting for a writer,
    // and wrap in AsyncFd so reads park on epoll instead of busy-looping.
    let fd = {
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) }
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("failed to open FIFO");
    }

    let std_file = unsafe { std::fs::File::from_raw_fd(fd) };
    let async_fd = AsyncFd::new(std_file).context("failed to register FIFO with epoll")?;

    let mut buf = vec![0u8; 4096];
    let mut line_buf = String::new();

    loop {
        let mut guard = async_fd.readable().await?;

        match guard.try_io(|inner| inner.get_ref().read(&mut buf)) {
            Ok(Ok(0)) => {
                // Shouldn't happen with O_RDWR (we hold a write end), but
                // if it does just wait for the next readability notification.
                continue;
            }
            Ok(Ok(n)) => {
                line_buf.push_str(&String::from_utf8_lossy(&buf[..n]));
                while let Some(pos) = line_buf.find('\n') {
                    let line = line_buf[..pos].trim().to_lowercase();
                    line_buf.drain(..=pos);
                    dispatch(&line, &tx).await;
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("FIFO read error: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            Err(_would_block) => {
                // Spurious wake — epoll said readable but read returned EAGAIN.
                // guard is cleared, loop back to readable().await.
                continue;
            }
        }
    }
}

async fn dispatch(line: &str, tx: &mpsc::Sender<RemoteSshCommand>) {
    match line {
        "enable" => {
            tracing::info!("received remote-ssh enable command");
            let _ = tx.send(RemoteSshCommand::Enable).await;
        }
        "disable" => {
            tracing::info!("received remote-ssh disable command");
            let _ = tx.send(RemoteSshCommand::Disable).await;
        }
        other if !other.is_empty() => {
            tracing::warn!("unknown remote-ssh command: {other}");
        }
        _ => {}
    }
}
