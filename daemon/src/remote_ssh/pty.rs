use anyhow::{Context, Result};
use pty_process::OwnedWritePty;
use tokio::process::Child;

pub struct PtyPair {
    pub writer: OwnedWritePty,
    pub child: Child,
}

pub fn spawn_shell(
    cols: u32,
    rows: u32,
    term: &str,
) -> Result<(pty_process::OwnedReadPty, PtyPair)> {
    let (pty, pts) = pty_process::open().context("failed to open PTY")?;
    pty.resize(pty_process::Size::new(rows as u16, cols as u16))
        .context("failed to set initial PTY size")?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let child = pty_process::Command::new(&shell)
        .arg("-l")
        .env("TERM", term)
        .spawn(pts)
        .context("failed to spawn shell")?;

    let (reader, writer) = pty.into_split();
    Ok((reader, PtyPair { writer, child }))
}

pub fn resize(writer: &OwnedWritePty, cols: u32, rows: u32) {
    let _ = writer.resize(pty_process::Size::new(rows as u16, cols as u16));
}
