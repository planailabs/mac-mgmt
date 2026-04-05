use std::io::BufRead;
use std::path::{Path, PathBuf};
use tokio::task::JoinHandle;

const MAX_LOG_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
const MAX_ROTATED: usize = 3;

fn strip_ansi(s: &str) -> String {
    let stripped = strip_ansi_escapes::strip(s);
    String::from_utf8(stripped).unwrap_or_else(|_| s.to_string())
}

/// Rotate log file if it exceeds MAX_LOG_SIZE.
/// Keeps up to MAX_ROTATED old files: name.log.1, name.log.2, etc.
fn maybe_rotate(log_path: &Path) -> std::io::Result<bool> {
    let meta = match std::fs::metadata(log_path) {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };
    if meta.len() < MAX_LOG_SIZE {
        return Ok(false);
    }

    // Shift existing rotated files
    for i in (1..MAX_ROTATED).rev() {
        let from = rotated_path(log_path, i);
        let to = rotated_path(log_path, i + 1);
        if from.exists() {
            let _ = std::fs::rename(&from, &to);
        }
    }

    // Rotate current → .1
    let _ = std::fs::rename(log_path, rotated_path(log_path, 1));
    Ok(true)
}

fn rotated_path(base: &Path, n: usize) -> PathBuf {
    let mut p = base.as_os_str().to_owned();
    p.push(format!(".{n}"));
    PathBuf::from(p)
}

/// Capture stdout/stderr from a child process, log via tracing, and write to a log file.
/// Returns a JoinHandle that completes when the child's pipes close (process exits).
pub fn capture(
    service_name: &str,
    child: &mut std::process::Child,
    log_dir: &Path,
) -> JoinHandle<()> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let name = service_name.to_string();
    let log_dir = log_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        // Create log directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            tracing::error!("failed to create log dir {}: {e}", log_dir.display());
            return;
        }

        let log_path = log_dir.join(format!("{name}.log"));

        // Rotate if the log file is already large (e.g., from a previous run)
        let _ = maybe_rotate(&log_path);

        use std::io::Write;
        use std::sync::{Arc, Mutex};

        let log_file = Arc::new(Mutex::new(
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("failed to open log file {}: {e}", log_path.display());
                    return;
                }
            },
        ));
        let bytes_written = Arc::new(std::sync::atomic::AtomicU64::new(
            std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0),
        ));

        let log_file2 = Arc::clone(&log_file);
        let bytes_written2 = Arc::clone(&bytes_written);
        let log_path2 = log_path.clone();

        // Helper: write a line, rotate if needed, reopen file
        let write_line = move |file: &Arc<Mutex<std::fs::File>>,
                               bytes: &Arc<std::sync::atomic::AtomicU64>,
                               path: &Path,
                               line: &str| {
            let mut f = file.lock().unwrap();
            let n = writeln!(f, "{line}").map(|_| line.len() as u64 + 1).unwrap_or(0);
            let total = bytes.fetch_add(n, std::sync::atomic::Ordering::Relaxed) + n;
            if total >= MAX_LOG_SIZE {
                drop(f);
                if maybe_rotate(path).unwrap_or(false) {
                    if let Ok(new_f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                    {
                        *file.lock().unwrap() = new_f;
                        bytes.store(0, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        };

        // We need to read from both stdout and stderr concurrently.
        // Use threads since this is already spawn_blocking.
        let name_clone = name.clone();

        let stderr_thread = stderr.map(|stderr| {
            let name = name_clone.clone();
            let write = {
                let file = Arc::clone(&log_file2);
                let bytes = Arc::clone(&bytes_written2);
                let path = log_path2.clone();
                move |line: &str| write_line(&file, &bytes, &path, line)
            };
            std::thread::spawn(move || {
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            let line = strip_ansi(&line);
                            tracing::warn!("[{name}] {line}");
                            write(&format!("[stderr] {line}"));
                        }
                        Err(e) => {
                            tracing::debug!("[{name}] stderr read error: {e}");
                            break;
                        }
                    }
                }
            })
        });

        if let Some(stdout) = stdout {
            let write = {
                let file = Arc::clone(&log_file);
                let bytes = Arc::clone(&bytes_written);
                let path = log_path.clone();
                move |line: &str| write_line(&file, &bytes, &path, line)
            };
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let line = strip_ansi(&line);
                        tracing::info!("[{name}] {line}");
                        write(&format!("[stdout] {line}"));
                    }
                    Err(e) => {
                        tracing::debug!("[{name}] stdout read error: {e}");
                        break;
                    }
                }
            }
        }

        if let Some(t) = stderr_thread {
            let _ = t.join();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[tokio::test]
    async fn captures_stdout_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = Command::new("echo")
            .arg("hello")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("test_svc", &mut child, dir.path());
        handle.await.unwrap();

        let content = std::fs::read_to_string(dir.path().join("test_svc.log")).unwrap();
        assert!(
            content.contains("hello"),
            "log should contain 'hello', got: {content}"
        );
    }

    #[tokio::test]
    async fn captures_stderr_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = Command::new("sh")
            .args(["-c", "echo error_msg >&2"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("test_svc", &mut child, dir.path());
        handle.await.unwrap();

        let content = std::fs::read_to_string(dir.path().join("test_svc.log")).unwrap();
        assert!(
            content.contains("error_msg"),
            "log should contain 'error_msg', got: {content}"
        );
    }

    #[tokio::test]
    async fn creates_log_dir_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let log_dir = dir.path().join("nested/logs");
        let mut child = Command::new("echo")
            .arg("test")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("svc", &mut child, &log_dir);
        handle.await.unwrap();

        assert!(log_dir.join("svc.log").exists());
    }

    #[test]
    fn strip_ansi_removes_colors() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[1;32mbold green\x1b[0m"), "bold green");
        assert_eq!(strip_ansi("no escapes"), "no escapes");
        assert_eq!(strip_ansi(""), "");
        assert_eq!(strip_ansi("\x1b[38;5;196mext color\x1b[0m"), "ext color");
    }

    #[tokio::test]
    async fn strips_ansi_from_captured_output() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = Command::new("sh")
            .args(["-c", "printf '\\033[31mred text\\033[0m\\n'"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("ansi_svc", &mut child, dir.path());
        handle.await.unwrap();

        let content = std::fs::read_to_string(dir.path().join("ansi_svc.log")).unwrap();
        assert!(
            !content.contains("\x1b"),
            "log should not contain ANSI escapes, got: {content:?}"
        );
        assert!(content.contains("red text"), "log should contain stripped text");
    }

    #[tokio::test]
    async fn exits_cleanly_when_child_exits() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = Command::new("true")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("svc", &mut child, dir.path());
        // Should complete without hanging
        tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("capture should complete within 5 seconds")
            .unwrap();
    }
}
