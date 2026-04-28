use anyhow::{Context, Result};

use crate::types::EntityCategory;

const NER_SYSTEM_PROMPT: &str = r#"You are a PII/sensitive-information detector. Given a text chunk, extract ALL:
- Person names (first, last, full)
- Company/organization names
- Project codenames or internal product names
- Internal URLs (intranet, staging, dev environments)
- Proprietary terms or trade secrets

Respond ONLY with a JSON array. Each element:
{"text": "exact text as it appears", "category": "person|company|project|internal_url|proprietary"}

If nothing found, respond with [].
Do NOT explain. Do NOT add commentary. JSON array only."#;

#[derive(serde::Deserialize)]
struct NerEntity {
    text: String,
    category: String,
}

pub struct OllamaDetected {
    pub text: String,
    pub category: EntityCategory,
}

/// Call Ollama's chat API to extract named entities from a text chunk.
pub async fn detect_entities(
    host: &str,
    port: u16,
    model: &str,
    chunk: &str,
) -> Result<Vec<OllamaDetected>> {
    let url = format!("http://{host}:{port}/api/chat");
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": NER_SYSTEM_PROMPT },
            { "role": "user", "content": chunk },
        ],
        "format": "json",
        "stream": false,
        "options": {
            "temperature": 0.1,
        }
    });

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("failed to reach Ollama")?;

    let status = resp.status();
    let text = resp.text().await.context("failed to read Ollama response")?;

    if !status.is_success() {
        anyhow::bail!("Ollama returned {status}: {text}");
    }

    // Parse the chat response to get the assistant's message content.
    let chat_resp: serde_json::Value =
        serde_json::from_str(&text).context("invalid JSON from Ollama")?;
    let content = chat_resp
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("[]");

    // The content should be a JSON array of entities.
    // Sometimes the model wraps it in an object like {"entities": [...]}.
    let entities: Vec<NerEntity> = if let Ok(arr) = serde_json::from_str::<Vec<NerEntity>>(content)
    {
        arr
    } else if let Ok(obj) = serde_json::from_str::<serde_json::Value>(content) {
        // Try to extract an array from any top-level key.
        obj.as_object()
            .and_then(|o| o.values().next())
            .and_then(|v| serde_json::from_value::<Vec<NerEntity>>(v.clone()).ok())
            .unwrap_or_default()
    } else {
        tracing::warn!("Ollama NER returned unparseable content: {content}");
        Vec::new()
    };

    Ok(entities
        .into_iter()
        .filter(|e| !e.text.is_empty())
        .map(|e| OllamaDetected {
            text: e.text,
            category: match e.category.as_str() {
                "person" => EntityCategory::Person,
                "company" => EntityCategory::Company,
                "project" => EntityCategory::ProjectName,
                "internal_url" => EntityCategory::InternalUrl,
                "proprietary" => EntityCategory::Proprietary,
                _ => EntityCategory::Custom,
            },
        })
        .collect())
}
