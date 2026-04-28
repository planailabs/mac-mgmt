use std::path::PathBuf;
use std::sync::Arc;

/// Shared state for the cloud MCP server.
#[derive(Clone)]
pub struct SharedState {
    inner: Arc<Inner>,
}

struct Inner {
    pub litellm_url: String,
    pub litellm_key: Option<String>,
    pub ollama_host: String,
    pub ollama_port: u16,
    pub cleaner_dir: PathBuf,
    pub audit_dir: PathBuf,
    pub strict: bool,
}

impl SharedState {
    pub fn new(
        litellm_url: String,
        litellm_key: Option<String>,
        ollama_host: String,
        ollama_port: u16,
        cleaner_dir: PathBuf,
        audit_dir: PathBuf,
        strict: bool,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                litellm_url,
                litellm_key,
                ollama_host,
                ollama_port,
                cleaner_dir,
                audit_dir,
                strict,
            }),
        }
    }

    pub fn litellm_url(&self) -> &str {
        &self.inner.litellm_url
    }
    pub fn litellm_key(&self) -> Option<&str> {
        self.inner.litellm_key.as_deref()
    }
    pub fn ollama_url(&self) -> String {
        format!("http://{}:{}", self.inner.ollama_host, self.inner.ollama_port)
    }
    pub fn cleaner_dir(&self) -> &std::path::Path {
        &self.inner.cleaner_dir
    }
    pub fn audit_dir(&self) -> &std::path::Path {
        &self.inner.audit_dir
    }
    pub fn strict(&self) -> bool {
        self.inner.strict
    }
}
