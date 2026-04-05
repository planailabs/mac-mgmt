use std::collections::HashSet;
use std::sync::RwLock;
use std::time::Duration;

use crate::events::DaemonEvent;

pub struct Dispatcher {
    urls: RwLock<Vec<String>>,
    events: RwLock<Option<HashSet<String>>>,
}

impl Dispatcher {
    pub fn new(urls: Vec<String>, events: Option<Vec<String>>) -> Self {
        Self {
            urls: RwLock::new(urls),
            events: RwLock::new(events.map(|v| v.into_iter().collect())),
        }
    }

    pub fn reconfigure(&self, urls: Vec<String>, events: Option<Vec<String>>) {
        *self.urls.write().unwrap() = urls;
        *self.events.write().unwrap() = events.map(|v| v.into_iter().collect());
        tracing::info!("notification dispatcher reconfigured");
    }

    pub fn dispatch(&self, event: &DaemonEvent) {
        let urls = self.urls.read().unwrap();
        if urls.is_empty() {
            return;
        }
        let events = self.events.read().unwrap();
        if let Some(filter) = &*events {
            if !filter.contains(event.kind()) {
                return;
            }
        }
        let message = event.to_string();
        let urls = urls.clone();
        tokio::spawn(async move {
            let mut cmd = tokio::process::Command::new("apprise");
            cmd.arg("-b").arg(&message);
            for url in &urls {
                cmd.arg(url);
            }
            match tokio::time::timeout(Duration::from_secs(30), cmd.output()).await {
                Ok(Ok(o)) if o.status.success() => {
                    tracing::debug!("notification sent: {message}")
                }
                Ok(Ok(o)) => tracing::warn!(
                    "apprise failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                ),
                Ok(Err(e)) => {
                    tracing::warn!("apprise not found or failed to execute: {e}")
                }
                Err(_) => tracing::warn!("apprise timed out"),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_urls_is_noop() {
        let d = Dispatcher::new(vec![], None);
        // Should not panic or spawn anything
        d.dispatch(&DaemonEvent::DaemonStarted);
    }

    #[test]
    fn event_filter_blocks_unmatched() {
        let d = Dispatcher::new(
            vec!["test://url".into()],
            Some(vec!["service_crashed".into()]),
        );
        // DaemonStarted is not in the filter — we can't easily assert the spawn
        // didn't happen without a runtime, but we verify no panic
        d.dispatch(&DaemonEvent::DaemonStarted);
    }

    #[tokio::test]
    async fn event_filter_allows_matched() {
        let d = Dispatcher::new(
            vec!["test://url".into()],
            Some(vec!["daemon_started".into()]),
        );
        d.dispatch(&DaemonEvent::DaemonStarted);
    }

    #[tokio::test]
    async fn none_filter_allows_all() {
        let d = Dispatcher::new(vec!["test://url".into()], None);
        // All events should pass through when events filter is None
        d.dispatch(&DaemonEvent::DaemonStarted);
        d.dispatch(&DaemonEvent::DaemonStopped);
        d.dispatch(&DaemonEvent::ServiceCrashed {
            service: "test".into(),
            exit_code: Some(1),
        });
    }
}
