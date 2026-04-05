use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

fn default_log_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/root"))
        .join(".local/share/mac-mgmt/logs")
}

fn resolve_log_dir(log_dir: Option<&str>) -> PathBuf {
    log_dir.map(PathBuf::from).unwrap_or_else(default_log_dir)
}

/// Read the last `n` lines from a file.
fn tail_file(path: &Path, n: usize) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().collect::<Result<_, _>>()
        .with_context(|| format!("failed to read {}", path.display()))?;

    let start = all_lines.len().saturating_sub(n);
    Ok(all_lines[start..].to_vec())
}

/// Follow a file for new content (like tail -f).
fn follow_file(path: &Path) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;

    // Seek to end
    file.seek(SeekFrom::End(0))?;
    let mut reader = BufReader::new(file);

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data, sleep briefly
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Ok(_) => {
                print!("{line}");
                std::io::stdout().flush()?;
            }
            Err(e) => {
                return Err(e).context("read error");
            }
        }
    }
}

pub fn tail_logs(
    service: Option<&str>,
    lines: usize,
    follow: bool,
    log_dir_override: Option<&str>,
) -> Result<()> {
    let log_dir = resolve_log_dir(log_dir_override);

    if !log_dir.exists() {
        anyhow::bail!("log directory not found at {}, is the daemon running?", log_dir.display());
    }

    if let Some(svc) = service {
        let log_path = log_dir.join(format!("{svc}.log"));
        if !log_path.exists() {
            anyhow::bail!("no log file for service '{svc}'");
        }

        // Print last N lines
        let tail = tail_file(&log_path, lines)?;
        for line in &tail {
            println!("{line}");
        }

        if follow {
            follow_file(&log_path)?;
        }
    } else {
        // All services: find all .log files
        let mut found = false;
        let entries = std::fs::read_dir(&log_dir)
            .context("failed to read log directory")?;

        let mut log_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "log"))
            .collect();
        log_files.sort();

        for log_path in &log_files {
            let svc_name = log_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            let tail = tail_file(log_path, lines)?;
            if !tail.is_empty() {
                found = true;
                println!("=== {svc_name} ===");
                for line in &tail {
                    println!("{line}");
                }
                println!();
            }
        }

        if !found {
            println!("No log files found.");
        }

        if follow {
            anyhow::bail!("follow mode requires a specific service (-s)");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_file_returns_last_n_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let content: String = (1..=100).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&path, &content).unwrap();

        let lines = tail_file(&path, 5).unwrap();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "line 96");
        assert_eq!(lines[4], "line 100");
    }

    #[test]
    fn tail_file_fewer_lines_than_requested() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        std::fs::write(&path, "line 1\nline 2\n").unwrap();

        let lines = tail_file(&path, 50).unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn tail_file_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        std::fs::write(&path, "").unwrap();

        let lines = tail_file(&path, 50).unwrap();
        assert!(lines.is_empty());
    }

    #[test]
    fn tail_logs_specific_service() {
        let dir = tempfile::tempdir().unwrap();
        let content = "line 1\nline 2\nline 3\n";
        std::fs::write(dir.path().join("ollama.log"), content).unwrap();

        // Should not error
        tail_logs(Some("ollama"), 10, false, Some(dir.path().to_str().unwrap())).unwrap();
    }

    #[test]
    fn tail_logs_nonexistent_service() {
        let dir = tempfile::tempdir().unwrap();
        let err = tail_logs(Some("nonexistent"), 10, false, Some(dir.path().to_str().unwrap())).unwrap_err();
        assert!(err.to_string().contains("no log file for service"), "got: {err}");
    }

    #[test]
    fn tail_logs_no_log_dir() {
        let err = tail_logs(None, 10, false, Some("/tmp/nonexistent_mac_mgmt_test_dir_xyz")).unwrap_err();
        assert!(err.to_string().contains("log directory not found"), "got: {err}");
    }

    #[test]
    fn tail_logs_all_services() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ollama.log"), "ollama line\n").unwrap();
        std::fs::write(dir.path().join("openclaw.log"), "openclaw line\n").unwrap();

        tail_logs(None, 50, false, Some(dir.path().to_str().unwrap())).unwrap();
    }
}
