use anyhow::Result;
use std::path::Path;

use crate::types::AuditEntry;

/// Append an audit entry to the daily JSONL file.
pub fn log_entry(audit_dir: &Path, entry: &AuditEntry) -> Result<()> {
    let filename = entry.timestamp.format("%Y-%m-%d").to_string() + ".jsonl";
    let path = audit_dir.join(filename);

    let line = serde_json::to_string(entry)? + "\n";
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?
        .write_all(line.as_bytes())?;

    Ok(())
}

use std::io::Write;

/// Read the most recent audit entries, up to `limit`.
pub fn read_recent(audit_dir: &Path, limit: u32) -> Result<Vec<AuditEntry>> {
    let mut files: Vec<_> = std::fs::read_dir(audit_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
        .collect();

    // Sort by filename descending (most recent first).
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    let mut entries = Vec::new();
    for file in files {
        if entries.len() >= limit as usize {
            break;
        }
        let content = std::fs::read_to_string(file.path())?;
        let mut file_entries: Vec<AuditEntry> = content
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        // Reverse so most recent entry in file comes first.
        file_entries.reverse();
        entries.extend(file_entries);
    }

    entries.truncate(limit as usize);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry() -> AuditEntry {
        AuditEntry {
            timestamp: chrono::Utc::now(),
            provider: "http://localhost:4100".into(),
            model: "anthropic/claude-sonnet-4-6".into(),
            prompt_sha256: "abcdef1234567890".into(),
            prompt_length_chars: 500,
            response_length_chars: 1200,
            cleaner_session_id: Some("sess-123".into()),
            tokens_in: Some(100),
            tokens_out: Some(250),
        }
    }

    #[test]
    fn write_and_read_audit_log() {
        let dir = tempfile::tempdir().unwrap();

        let entry1 = sample_entry();
        log_entry(dir.path(), &entry1).unwrap();

        let mut entry2 = sample_entry();
        entry2.model = "openai/gpt-5.4".into();
        entry2.cleaner_session_id = None;
        log_entry(dir.path(), &entry2).unwrap();

        let entries = read_recent(dir.path(), 10).unwrap();
        assert_eq!(entries.len(), 2);
        // Most recent first (reversed within the file).
        assert_eq!(entries[0].model, "openai/gpt-5.4");
        assert_eq!(entries[1].model, "anthropic/claude-sonnet-4-6");
    }

    #[test]
    fn limit_truncates() {
        let dir = tempfile::tempdir().unwrap();

        for _ in 0..5 {
            log_entry(dir.path(), &sample_entry()).unwrap();
        }

        let entries = read_recent(dir.path(), 3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let entries = read_recent(dir.path(), 10).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn audit_entry_roundtrip_json() {
        let entry = sample_entry();
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model, entry.model);
        assert_eq!(parsed.prompt_length_chars, 500);
        assert_eq!(parsed.tokens_in, Some(100));
        assert_eq!(parsed.cleaner_session_id, Some("sess-123".into()));
    }
}
