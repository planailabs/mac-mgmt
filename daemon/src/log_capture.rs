use std::io::BufRead;
use tokio::task::JoinHandle;

use crate::log_buffer::LogBuffer;

fn strip_ansi(s: &str) -> String {
    let stripped = strip_ansi_escapes::strip(s);
    String::from_utf8(stripped).unwrap_or_else(|_| s.to_string())
}

fn drain_lines(reader: impl BufRead, service_name: &str, is_stderr: bool, buf: &LogBuffer) {
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let clean = strip_ansi(&line);
        if is_stderr {
            tracing::warn!(target: "service", "[{service_name}] {clean}");
        } else {
            tracing::info!(target: "service", "[{service_name}] {clean}");
        }
        buf.push(format!("[{service_name}] {clean}"));
    }
}

/// Capture stdout/stderr from a child process and push lines to the log buffer.
/// Lines are prefixed with the service name.
pub fn capture(
    service_name: &str,
    child: &mut std::process::Child,
    buf: &LogBuffer,
) -> JoinHandle<()> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let name = service_name.to_string();
    let buf = buf.clone();

    tokio::task::spawn_blocking(move || {
        let stderr_thread = stderr.map(|stderr| {
            let name = name.clone();
            let buf = buf.clone();
            std::thread::spawn(move || {
                drain_lines(std::io::BufReader::new(stderr), &name, true, &buf);
            })
        });

        if let Some(stdout) = stdout {
            drain_lines(std::io::BufReader::new(stdout), &name, false, &buf);
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
    async fn captures_stdout() {
        let buf = LogBuffer::new();
        let mut child = Command::new("echo")
            .arg("hello")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("test_svc", &mut child, &buf);
        handle.await.unwrap();

        let lines = buf.all();
        assert!(
            lines.iter().any(|l| l.contains("[test_svc] hello")),
            "expected '[test_svc] hello', got: {lines:?}"
        );
    }

    #[tokio::test]
    async fn captures_stderr() {
        let buf = LogBuffer::new();
        let mut child = Command::new("sh")
            .args(["-c", "echo error_msg >&2"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("svc", &mut child, &buf);
        handle.await.unwrap();

        let lines = buf.all();
        assert!(
            lines.iter().any(|l| l.contains("[svc] error_msg")),
            "expected '[svc] error_msg', got: {lines:?}"
        );
    }

    #[tokio::test]
    async fn strips_ansi() {
        let buf = LogBuffer::new();
        let mut child = Command::new("sh")
            .args(["-c", "printf '\\033[31mred\\033[0m\\n'"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let handle = capture("svc", &mut child, &buf);
        handle.await.unwrap();

        let lines = buf.all();
        assert!(
            !lines.iter().any(|l| l.contains("\x1b")),
            "should not contain ANSI escapes: {lines:?}"
        );
    }
}
