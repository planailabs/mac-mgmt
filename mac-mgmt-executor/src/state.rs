use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backend::IncusBackend;
use crate::types::{InstanceInfo, OsImage};

#[derive(Clone)]
pub struct SharedState {
    inner: Arc<RwLock<StateInner>>,
    pub backend: Arc<dyn IncusBackend>,
}

struct StateInner {
    session_prefix: String,
    instances: HashMap<String, InstanceInfo>,
    last_created: Option<String>,
    image_cache: Option<Vec<OsImage>>,
}

impl SharedState {
    pub fn new(backend: Arc<dyn IncusBackend>) -> Self {
        let session_id = format!("{:04x}", rand::random::<u16>());
        let session_prefix = format!("mcp-exec-{session_id}-");
        tracing::info!("session prefix: {session_prefix}");

        Self {
            inner: Arc::new(RwLock::new(StateInner {
                session_prefix,
                instances: HashMap::new(),
                last_created: None,
                image_cache: None,
            })),
            backend,
        }
    }

    pub async fn session_prefix(&self) -> String {
        self.inner.read().await.session_prefix.clone()
    }

    pub async fn generate_name(&self) -> String {
        let suffix = format!("{:04x}", rand::random::<u16>());
        let prefix = self.inner.read().await.session_prefix.clone();
        format!("{prefix}{suffix}")
    }

    pub async fn add_instance(&self, info: InstanceInfo) {
        let mut inner = self.inner.write().await;
        let name = info.name.clone();
        inner.instances.insert(name.clone(), info);
        inner.last_created = Some(name);
    }

    pub async fn remove_instance(&self, name: &str) -> Option<InstanceInfo> {
        let mut inner = self.inner.write().await;
        let removed = inner.instances.remove(name);
        if inner.last_created.as_deref() == Some(name) {
            inner.last_created = inner.instances.keys().last().cloned();
        }
        removed
    }

    pub async fn resolve_name(&self, name: Option<&str>) -> Result<String, String> {
        match name {
            Some(n) => {
                let inner = self.inner.read().await;
                if inner.instances.contains_key(n) {
                    Ok(n.to_string())
                } else {
                    Err(format!("no container named '{n}' in this session"))
                }
            }
            None => {
                let inner = self.inner.read().await;
                inner
                    .last_created
                    .clone()
                    .ok_or_else(|| "no container created yet -- use system_create first".to_string())
            }
        }
    }

    pub async fn list_instances(&self) -> Vec<InstanceInfo> {
        let inner = self.inner.read().await;
        let mut list: Vec<_> = inner.instances.values().cloned().collect();
        list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        list
    }

    pub async fn get_or_fetch_images(&self) -> anyhow::Result<Vec<OsImage>> {
        // Check cache first.
        {
            let inner = self.inner.read().await;
            if let Some(cached) = &inner.image_cache {
                return Ok(cached.clone());
            }
        }

        // Fetch and cache.
        let images = self.backend.image_list().await?;
        {
            let mut inner = self.inner.write().await;
            inner.image_cache = Some(images.clone());
        }
        Ok(images)
    }

    pub async fn take_all_instance_names(&self) -> Vec<String> {
        let mut inner = self.inner.write().await;
        let names: Vec<String> = inner.instances.keys().cloned().collect();
        inner.instances.clear();
        inner.last_created = None;
        names
    }
}
