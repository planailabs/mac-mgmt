use std::io::BufRead;
use std::path::Path;
use tokio::task::JoinHandle;

fn strip_ansi(s: &str) -> String {
    let stripped = strip_ansi_escapes::strip(s);
    String::from_utf8(stripped).unwrap_or_else(|_| s.to_string())
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
        let mut log_file = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("failed to open log file {}: {e}", log_path.display());
                return;
            }
        };

        use std::io::Write;

        // We need to read from both stdout and stderr concurrently.
        // Use threads since this is already spawn_blocking.
        let name_clone = name.clone();
        let mut log_file_clone = log_file.try_clone().unwrap_or_else(|e| {
            tracing::error!("failed to clone log file handle: {e}");
            // Return a handle anyway -- writes will fail
            std::fs::OpenOptions::new()
                .append(true)
                .open(&log_path)
                .unwrap()
        });

        let stderr_thread = stderr.map(|stderr| {
            let name = name_clone.clone();
            std::thread::spawn(move || {
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            let line = strip_ansi(&line);
                            tracing::warn!("[{name}] {line}");
                            let _ = writeln!(log_file_clone, "[stderr] {line}");
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
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        let line = strip_ansi(&line);
                        tracing::info!("[{name}] {line}");
                        let _ = writeln!(log_file, "[stdout] {line}");
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
