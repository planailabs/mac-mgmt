pub mod apprise;
pub mod mcporter;
pub mod nexa;
pub mod ollama;
pub mod openclaw;

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Send a blocking HTTP GET and return the response body as a string.
/// Uses raw TCP to avoid creating a nested tokio runtime (reqwest::blocking
/// panics when dropped inside an async context).
pub fn http_get(host: &str, port: u16, path: &str) -> Result<String> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr)
        .with_context(|| format!("connect to {addr} failed"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes())
        .with_context(|| format!("write to {addr} failed"))?;

    let reader = BufReader::new(&stream);
    let mut lines = Vec::new();
    for line in reader.lines() {
        match line {
            Ok(l) => lines.push(l),
            Err(_) => break,
        }
    }

    // Find body after the blank line separating headers from body
    let body_start = lines.iter().position(|l| l.is_empty())
        .context("invalid HTTP response (no header/body separator)")?;

    // Handle chunked transfer encoding
    let is_chunked = lines[..body_start].iter()
        .any(|l| l.to_lowercase().contains("transfer-encoding: chunked"));

    let body_lines = &lines[body_start + 1..];

    if is_chunked {
        // Parse chunked encoding: alternating size lines and data lines
        let mut body = String::new();
        let mut i = 0;
        while i < body_lines.len() {
            let size = usize::from_str_radix(body_lines[i].trim(), 16).unwrap_or(0);
            if size == 0 {
                break;
            }
            if i + 1 < body_lines.len() {
                body.push_str(&body_lines[i + 1]);
            }
            i += 2;
        }
        Ok(body)
    } else {
        Ok(body_lines.join("\n"))
    }
}
