use super::types::*;
use super::{AiProxyState, BackendEndpoint};
use std::sync::Arc;

/// Resolved backend for a model request.
pub struct ResolvedBackend {
    pub endpoint: BackendEndpoint,
    pub backend_name: String,
}

/// Resolve which backend to use for a given model name.
pub async fn resolve_backend(
    state: &Arc<AiProxyState>,
    _model: &str,
) -> Option<ResolvedBackend> {
    let backends = state.backends.read().await;

    // Check unsloth first (more specific — only handles its configured model)
    if let Some(ref unsloth) = backends.unsloth {
        // Unsloth is used if model matches or if it's the only backend
        if backends.ollama.is_none() {
            return Some(ResolvedBackend {
                endpoint: unsloth.clone(),
                backend_name: "unsloth".to_string(),
            });
        }
    }

    // Default to Ollama (handles arbitrary models, will pull if needed)
    if let Some(ref ollama) = backends.ollama {
        return Some(ResolvedBackend {
            endpoint: ollama.clone(),
            backend_name: "ollama".to_string(),
        });
    }

    // Fall back to Unsloth
    if let Some(ref unsloth) = backends.unsloth {
        return Some(ResolvedBackend {
            endpoint: unsloth.clone(),
            backend_name: "unsloth".to_string(),
        });
    }

    None
}

/// Proxy a non-streaming chat completion request.
pub async fn proxy_chat_completion(
    client: &reqwest::Client,
    backend: &ResolvedBackend,
    request: &ChatCompletionRequest,
) -> Result<ChatCompletionResponse, ProxyError> {
    let url = format!("{}/v1/chat/completions", backend.endpoint.base_url());

    let resp = client
        .post(&url)
        .json(request)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .map_err(|e| ProxyError::Backend(format!("request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ProxyError::BackendStatus(status.as_u16(), body));
    }

    resp.json::<ChatCompletionResponse>()
        .await
        .map_err(|e| ProxyError::Backend(format!("invalid response: {e}")))
}

/// Start a streaming chat completion request, returning the raw byte stream.
pub async fn proxy_chat_completion_stream(
    client: &reqwest::Client,
    backend: &ResolvedBackend,
    request: &ChatCompletionRequest,
) -> Result<reqwest::Response, ProxyError> {
    let url = format!("{}/v1/chat/completions", backend.endpoint.base_url());

    let resp = client
        .post(&url)
        .json(request)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await
        .map_err(|e| ProxyError::Backend(format!("stream request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ProxyError::BackendStatus(status.as_u16(), body));
    }

    Ok(resp)
}

/// Fetch available models from all backends.
pub async fn list_models(state: &Arc<AiProxyState>) -> Vec<Model> {
    let backends = state.backends.read().await;
    let mut models = Vec::new();

    if let Some(ref ollama) = backends.ollama {
        match fetch_ollama_models(&state.client, ollama).await {
            Ok(m) => models.extend(m),
            Err(e) => tracing::debug!("failed to list ollama models: {e}"),
        }
    }

    if let Some(ref unsloth) = backends.unsloth {
        match fetch_openai_models(&state.client, unsloth).await {
            Ok(m) => models.extend(m),
            Err(e) => tracing::debug!("failed to list unsloth models: {e}"),
        }
    }

    models
}

async fn fetch_ollama_models(
    client: &reqwest::Client,
    endpoint: &BackendEndpoint,
) -> Result<Vec<Model>, String> {
    let url = format!("{}/api/tags", endpoint.base_url());
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("ollama /api/tags failed: {e}"))?;

    #[derive(serde::Deserialize)]
    struct OllamaModels {
        models: Vec<OllamaModel>,
    }
    #[derive(serde::Deserialize)]
    struct OllamaModel {
        name: String,
        #[serde(default)]
        modified_at: Option<String>,
    }

    let body: OllamaModels = resp
        .json()
        .await
        .map_err(|e| format!("ollama /api/tags parse: {e}"))?;

    Ok(body
        .models
        .into_iter()
        .map(|m| {
            let created = m
                .modified_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp())
                .unwrap_or(0);
            Model {
                id: m.name,
                object: "model".to_string(),
                created,
                owned_by: "ollama".to_string(),
            }
        })
        .collect())
}

async fn fetch_openai_models(
    client: &reqwest::Client,
    endpoint: &BackendEndpoint,
) -> Result<Vec<Model>, String> {
    let url = format!("{}/v1/models", endpoint.base_url());
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("models endpoint failed: {e}"))?;

    let body: ModelsResponse = resp
        .json()
        .await
        .map_err(|e| format!("models parse: {e}"))?;

    Ok(body.data)
}

#[derive(Debug)]
pub enum ProxyError {
    Backend(String),
    BackendStatus(u16, String),
    NoBackend,
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyError::Backend(msg) => write!(f, "backend error: {msg}"),
            ProxyError::BackendStatus(status, body) => {
                write!(f, "backend returned {status}: {body}")
            }
            ProxyError::NoBackend => write!(f, "no backend available for this model"),
        }
    }
}
