use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

/// Watch a config file for changes. Sends a message on the channel when the file is modified.
/// Debounces with a 1-second delay to handle editors that write temp files.
pub fn watch(
    config_path: &Path,
    tx: mpsc::Sender<()>,
) -> notify::Result<RecommendedWatcher> {
    let path = config_path.to_path_buf();
    let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    ) {
                        // Check if the event is for our config file
                        if event.paths.iter().any(|p| p.ends_with(path.file_name().unwrap_or_default())) {
                            let _ = tx.try_send(());
                        }
                    }
                }
                Err(e) => tracing::warn!("config watcher error: {e}"),
            }
        },
        notify::Config::default(),
    )?;

    watcher.watch(&parent, RecursiveMode::NonRecursive)?;

    Ok(watcher)
}

/// Receive config change events with debouncing.
/// Returns when a config change is detected (after 1s debounce).
pub async fn recv_debounced(rx: &mut mpsc::Receiver<()>) {
    // Wait for first event
    rx.recv().await;
    // Debounce: drain any additional events within 1 second
    loop {
        match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
            Ok(Some(())) => continue, // More events, keep debouncing
            _ => break,               // Timeout or channel closed
        }
    }
}
