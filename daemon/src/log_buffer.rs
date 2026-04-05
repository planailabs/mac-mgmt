use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const MAX_LINES: usize = 10_000;

/// A thread-safe ring buffer of log lines.
#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<String>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LINES))),
        }
    }

    /// Push a line into the buffer. Oldest lines are dropped when full.
    pub fn push(&self, line: String) {
        let mut buf = self.inner.lock().unwrap();
        if buf.len() >= MAX_LINES {
            buf.pop_front();
        }
        buf.push_back(line);
    }

    /// Get the last `n` lines (or all if n > buffer size).
    pub fn tail(&self, n: usize) -> Vec<String> {
        let buf = self.inner.lock().unwrap();
        let start = buf.len().saturating_sub(n);
        buf.iter().skip(start).cloned().collect()
    }

    /// Get all lines.
    pub fn all(&self) -> Vec<String> {
        let buf = self.inner.lock().unwrap();
        buf.iter().cloned().collect()
    }

    /// Get lines added after a given index. Returns (new_index, lines).
    pub fn since(&self, after: usize) -> (usize, Vec<String>) {
        let buf = self.inner.lock().unwrap();
        let total = buf.len();
        if after >= total {
            return (total, vec![]);
        }
        let lines = buf.iter().skip(after).cloned().collect();
        (total, lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_tail() {
        let buf = LogBuffer::new();
        for i in 0..5 {
            buf.push(format!("line {i}"));
        }
        let last3 = buf.tail(3);
        assert_eq!(last3, vec!["line 2", "line 3", "line 4"]);
    }

    #[test]
    fn ring_buffer_drops_oldest() {
        let buf = LogBuffer::new();
        for i in 0..MAX_LINES + 5 {
            buf.push(format!("line {i}"));
        }
        let all = buf.all();
        assert_eq!(all.len(), MAX_LINES);
        assert_eq!(all[0], "line 5");
    }

    #[test]
    fn since_returns_new_lines() {
        let buf = LogBuffer::new();
        buf.push("a".into());
        buf.push("b".into());
        let (idx, lines) = buf.since(0);
        assert_eq!(idx, 2);
        assert_eq!(lines, vec!["a", "b"]);

        buf.push("c".into());
        let (idx2, lines2) = buf.since(idx);
        assert_eq!(idx2, 3);
        assert_eq!(lines2, vec!["c"]);
    }
}
