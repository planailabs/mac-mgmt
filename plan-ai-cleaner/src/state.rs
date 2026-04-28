use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::storage::SessionStore;
use crate::types::{EntityCategory, RedactionEntity};

/// Shared state for the MCP server.
#[derive(Clone)]
pub struct SharedState {
    inner: Arc<Inner>,
}

struct Inner {
    pub store: SessionStore,
    /// Per-category counters for generating unique placeholder IDs within a session.
    counters: RwLock<HashMap<String, HashMap<EntityCategory, usize>>>,
}

impl SharedState {
    pub fn new(store: SessionStore) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                counters: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub fn store(&self) -> &SessionStore {
        &self.inner.store
    }

    /// Allocate a new placeholder ID for the given session and category.
    /// Returns something like "PERSON_1", "EMAIL_2", etc.
    pub async fn next_placeholder(
        &self,
        session_id: &str,
        category: EntityCategory,
    ) -> String {
        let mut counters = self.inner.counters.write().await;
        let session_counters = counters
            .entry(session_id.to_string())
            .or_default();
        let count = session_counters.entry(category).or_insert(0);
        *count += 1;
        format!("{}_{}", category.prefix(), count)
    }

    /// Build entities from detected matches, deduplicating by original text.
    /// Entities that share the same text get the same placeholder.
    pub async fn build_entities(
        &self,
        session_id: &str,
        detections: Vec<(String, EntityCategory, crate::types::DetectionSource)>,
    ) -> Vec<RedactionEntity> {
        let mut entities = Vec::new();
        let mut seen: HashMap<String, String> = HashMap::new(); // original → placeholder_id

        for (text, category, source) in detections {
            if let Some(existing_id) = seen.get(&text) {
                // Already have this entity — skip (same placeholder).
                let _ = existing_id;
                continue;
            }

            let id = self.next_placeholder(session_id, category).await;
            let placeholder = format!("[{id}]");
            seen.insert(text.clone(), id.clone());

            entities.push(RedactionEntity {
                id,
                category,
                original: text,
                placeholder,
                source,
                approved: true, // Default to approved; user can remove via remove_ids.
            });
        }

        entities
    }

    /// Clear the counter state for a session (after deletion).
    pub async fn clear_session(&self, session_id: &str) {
        let mut counters = self.inner.counters.write().await;
        counters.remove(session_id);
    }

    /// Run GC on startup.
    pub fn gc(&self) -> usize {
        self.inner.store.gc()
    }
}
