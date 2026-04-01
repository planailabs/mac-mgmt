use anyhow::{Context, Result};
use std::ffi::CString;
use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;

use super::RemoteSshCommand;

fn fifo_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config/mac-mgmt/remote-ssh")
}

fn create_fifo(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).ok();
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let c_path = CString::new(path.to_str().context("non-UTF8 path")?)
        .context("path contains null byte")?;
    let ret = unsafe { libc::mkfifo(c_path.as_ptr(), 0o622) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("mkfifo failed for {}", path.display()));
    }
    Ok(())
}

pub async fn watch(tx: mpsc::Sender<RemoteSshCommand>) -> Result<()> {
    let path = fifo_path();
    create_fifo(&path)?;
    tracing::info!("FIFO created at {}", path.display());

    loop {
        // Open with O_RDWR to avoid blocking (standard FIFO trick)
        let fd = {
            let c_path = CString::new(path.to_str().unwrap()).unwrap();
            unsafe { libc::open(c_path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) }
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error())
                .context("failed to open FIFO");
        }

        let std_file = unsafe { std::fs::File::from_raw_fd(fd) };
        let file = tokio::fs::File::from_std(std_file);
        let reader = tokio::io::BufReader::new(file);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_lowercase();
            match line.as_str() {
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

        // lines returned None — FIFO closed by all writers, re-open
        tracing::debug!("FIFO EOF, re-opening");
    }
}

use std::os::unix::io::FromRawFd;
