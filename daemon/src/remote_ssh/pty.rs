use anyhow::{Context, Result};
use std::os::unix::io::{FromRawFd, RawFd};
use std::process::Stdio;
use tokio::process::{Child, Command};

pub struct PtyPair {
    pub master_fd: RawFd,
    pub child: Child,
}

pub fn spawn_shell(
    cols: u32,
    rows: u32,
    term: &str,
) -> Result<PtyPair> {
    let mut master: RawFd = 0;
    let mut slave: RawFd = 0;

    let ret = unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error()).context("openpty failed");
    }

    // Set terminal size
    let ws = libc::winsize {
        ws_row: rows as u16,
        ws_col: cols as u16,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe { libc::ioctl(master, libc::TIOCSWINSZ, &ws) };

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let slave_stdin = unsafe { Stdio::from_raw_fd(slave) };
    let slave_stdout = unsafe { Stdio::from_raw_fd(libc::dup(slave)) };
    let slave_stderr = unsafe { Stdio::from_raw_fd(libc::dup(slave)) };

    let child = unsafe {
        Command::new(&shell)
            .arg("-l")
            .env("TERM", term)
            .stdin(slave_stdin)
            .stdout(slave_stdout)
            .stderr(slave_stderr)
            .pre_exec(|| {
                // Create new session and set controlling terminal
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            })
            .spawn()
            .context("failed to spawn shell")?
    };

    // Close slave FD in parent — child owns it now
    unsafe { libc::close(slave) };

    Ok(PtyPair { master_fd: master, child })
}

pub fn resize(master_fd: RawFd, cols: u32, rows: u32) {
    let ws = libc::winsize {
        ws_row: rows as u16,
        ws_col: cols as u16,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe { libc::ioctl(master_fd, libc::TIOCSWINSZ, &ws) };
}
