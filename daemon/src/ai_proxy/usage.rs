use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub ts: DateTime<Utc>,
    pub key_hash: String,
    pub key_name: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub latency_ms: u64,
    pub backend: String,
}

impl UsageEvent {
    pub fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }
}

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

pub struct UsageTracker {
    events: RwLock<Vec<UsageEvent>>,
    path: PathBuf,
    /// Buffer of events not yet flushed to disk.
    pending: RwLock<Vec<UsageEvent>>,
}

impl UsageTracker {
    /// Create a new tracker, loading existing events from the JSONL file.
    pub fn new(path: PathBuf) -> Self {
        let events = Self::load_from_file(&path);
        Self {
            events: RwLock::new(events),
            path,
            pending: RwLock::new(Vec::new()),
        }
    }

    /// Record a new usage event.
    pub fn record(&self, event: UsageEvent) {
        if let Ok(mut events) = self.events.write() {
            events.push(event.clone());
        }
        if let Ok(mut pending) = self.pending.write() {
            pending.push(event);
        }
    }

    /// Sum tokens for a given key within the sliding window.
    pub fn tokens_in_window(&self, key_hash: &str, window: Duration) -> i64 {
        let cutoff = Utc::now() - chrono::Duration::from_std(window).unwrap_or_default();
        let events = self.events.read().unwrap();
        // Scan from end (events are append-ordered by time)
        events
            .iter()
            .rev()
            .take_while(|e| e.ts >= cutoff)
            .filter(|e| e.key_hash == key_hash)
            .map(|e| e.total_tokens())
            .sum()
    }

    /// Remaining budget for a key. Returns i64::MAX if budget is 0 (unlimited).
    pub fn remaining_budget(&self, key_hash: &str, budget: i64, window: Duration) -> i64 {
        if budget == 0 {
            return i64::MAX;
        }
        budget - self.tokens_in_window(key_hash, window)
    }

    /// Get recent events for a key within a time window.
    pub fn recent_events(&self, key_hash: &str, window: Duration, limit: usize) -> Vec<UsageEvent> {
        let cutoff = Utc::now() - chrono::Duration::from_std(window).unwrap_or_default();
        let events = self.events.read().unwrap();
        events
            .iter()
            .rev()
            .filter(|e| e.ts >= cutoff && e.key_hash == key_hash)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Flush pending events to the JSONL file.
    pub fn flush(&self) {
        let to_write: Vec<UsageEvent> = {
            let mut pending = match self.pending.write() {
                Ok(p) => p,
                Err(_) => return,
            };
            std::mem::take(&mut *pending)
        };

        if to_write.is_empty() {
            return;
        }

        // Rotate if file is too large
        self.maybe_rotate();

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path);

        let mut file = match file {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("failed to open usage file {}: {e}", self.path.display());
                // Put events back
                if let Ok(mut pending) = self.pending.write() {
                    pending.extend(to_write);
                }
                return;
            }
        };

        for event in &to_write {
            if let Ok(line) = serde_json::to_string(event) {
                let _ = writeln!(file, "{line}");
            }
        }
        let _ = file.flush();
    }

    /// Remove events older than the max window * 2 to bound growth.
    pub fn compact(&self, max_window: Duration) {
        let cutoff = Utc::now()
            - chrono::Duration::from_std(max_window * 2).unwrap_or(chrono::Duration::days(2));
        if let Ok(mut events) = self.events.write() {
            events.retain(|e| e.ts >= cutoff);
        }
    }

    fn maybe_rotate(&self) {
        let size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if size > MAX_FILE_SIZE {
            let backup = self.path.with_extension("jsonl.1");
            let _ = std::fs::rename(&self.path, &backup);
            tracing::info!(
                "rotated usage log {} -> {} ({size} bytes)",
                self.path.display(),
                backup.display()
            );
        }
    }

    fn load_from_file(path: &Path) -> Vec<UsageEvent> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut events = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<UsageEvent>(line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    tracing::debug!("skipping malformed usage event: {e}");
                }
            }
        }
        tracing::info!(
            "loaded {} usage events from {}",
            events.len(),
            path.display()
        );
        events
    }
}
