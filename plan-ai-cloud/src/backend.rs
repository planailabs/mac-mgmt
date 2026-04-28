use anyhow::{Context, Result};

/// Response from a chat completion call.
pub struct CompletionResponse {
    pub text: String,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
}

/// Send a chat completion request to an OpenAI-compatible endpoint
/// (LiteLLM or Ollama's /v1/chat/completions).
pub async fn chat_completion(
    base_url: &str,
    api_key: Option<&str>,
    model: Option<&str>,
    prompt: &str,
    system: Option<&str>,
    max_tokens: u32,
    temperature: f32,
) -> Result<CompletionResponse> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let body = build_request_body(model, prompt, system, max_tokens, temperature);

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }

    let resp = req.send().await.context("failed to reach LLM endpoint")?;
    let status = resp.status();
    let text = resp.text().await.context("failed to read LLM response")?;

    if !status.is_success() {
        anyhow::bail!("LLM endpoint returned {status}: {text}");
    }

    let json: serde_json::Value = serde_json::from_str(&text).context("invalid JSON response")?;

    Ok(parse_response(&json))
}

/// Build the request body for an OpenAI-compatible chat completion.
pub fn build_request_body(
    model: Option<&str>,
    prompt: &str,
    system: Option<&str>,
    max_tokens: u32,
    temperature: f32,
) -> serde_json::Value {
    let mut messages = Vec::new();
    if let Some(sys) = system {
        messages.push(serde_json::json!({ "role": "system", "content": sys }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": prompt }));

    let mut body = serde_json::json!({
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });
    if let Some(m) = model {
        body["model"] = serde_json::json!(m);
    }
    body
}

/// Parse an OpenAI-compatible chat completion response.
pub fn parse_response(json: &serde_json::Value) -> CompletionResponse {
    let content = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    let usage = json.get("usage");
    let tokens_in = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|t| t.as_u64())
        .map(|t| t as u32);
    let tokens_out = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|t| t.as_u64())
        .map(|t| t as u32);

    CompletionResponse {
        text: content,
        tokens_in,
        tokens_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_body_without_model() {
        let body = build_request_body(None, "Hello", None, 100, 0.5);
        assert!(body.get("model").is_none());
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello");
        assert_eq!(body["max_tokens"], 100);
    }

    #[test]
    fn build_body_with_system_and_model() {
        let body = build_request_body(
            Some("anthropic/claude-sonnet-4-6"),
            "Analyze this",
            Some("You are a helpful assistant"),
            4096,
            0.7,
        );
        assert_eq!(body["model"], "anthropic/claude-sonnet-4-6");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");
    }

    #[test]
    fn parse_openai_response() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "Hello! How can I help?"
                }
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 25,
            }
        });

        let resp = parse_response(&json);
        assert_eq!(resp.text, "Hello! How can I help?");
        assert_eq!(resp.tokens_in, Some(10));
        assert_eq!(resp.tokens_out, Some(25));
    }

    #[test]
    fn parse_response_missing_usage() {
        let json = serde_json::json!({
            "choices": [{
                "message": { "content": "test" }
            }]
        });

        let resp = parse_response(&json);
        assert_eq!(resp.text, "test");
        assert_eq!(resp.tokens_in, None);
        assert_eq!(resp.tokens_out, None);
    }

    #[test]
    fn parse_empty_response() {
        let json = serde_json::json!({});
        let resp = parse_response(&json);
        assert_eq!(resp.text, "");
    }

    #[test]
    fn url_construction() {
        let url = format!(
            "{}/v1/chat/completions",
            "http://localhost:4100".trim_end_matches('/')
        );
        assert_eq!(url, "http://localhost:4100/v1/chat/completions");

        let url2 = format!(
            "{}/v1/chat/completions",
            "http://localhost:4100/".trim_end_matches('/')
        );
        assert_eq!(url2, "http://localhost:4100/v1/chat/completions");
    }
}
