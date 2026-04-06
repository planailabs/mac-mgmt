use std::io::BufRead;
use tokio::task::JoinHandle;

use crate::log_buffer::LogBuffer;

fn strip_ansi(s: &str) -> String {
    let stripped = strip_ansi_escapes::strip(s);
    String::from_utf8(stripped).unwrap_or_else(|_| s.to_string())
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
        let name2 = name.clone();
        let buf2 = buf.clone();

        let stderr_thread = stderr.map(|stderr| {
            let name = name2;
            let buf = buf2;
            std::thread::spawn(move || {
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            eprintln!("[{name}] {line}");
                            let clean = strip_ansi(&line);
                            buf.push(format!("[{name}] {clean}"));
                        }
                        Err(_) => break,
                    }
                }
            })
        });

        if let Some(stdout) = stdout {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        println!("[{name}] {line}");
                        let clean = strip_ansi(&line);
                        buf.push(format!("[{name}] {clean}"));
                    }
                    Err(_) => break,
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
