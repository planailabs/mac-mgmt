use rocket::http::{ContentType, Status};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::{Either, State, get, post, routes};
use std::sync::Arc;

use super::auth::AuthedKey;
use super::backend::{self, ProxyError};
use super::types::*;
use super::usage::UsageEvent;
use super::AiProxyState;

#[post("/v1/chat/completions", data = "<body>")]
async fn chat_completions(
    auth: AuthedKey,
    state: &State<Arc<AiProxyState>>,
    body: Json<ChatCompletionRequest>,
) -> Result<
    Either<Json<ChatCompletionResponse>, EventStream![Event + 'static]>,
    (Status, Json<ErrorResponse>),
> {
    let start = std::time::Instant::now();
    let _job_guard = state.track_job();
    let request = body.into_inner();
    let model = request.model.clone();
    let is_stream = request.stream.unwrap_or(false);

    let resolved = backend::resolve_backend(state, &model)
        .await
        .ok_or_else(|| {
            (
                Status::BadGateway,
                Json(ErrorResponse::new(
                    "no backend available",
                    "server_error",
                    Some("no_backend"),
                )),
            )
        })?;

    let backend_name = resolved.backend_name().to_string();

    // Handle peer forwarding via libp2p
    #[cfg(feature = "relay")]
    if let backend::ResolvedBackend::Peer { peer_id, .. } = &resolved {
        if let Some(ref tx) = state.p2p_request_tx {
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let body = serde_json::to_value(&request).map_err(|e| {
                (
                    Status::InternalServerError,
                    Json(ErrorResponse::new(
                        &format!("serialize error: {e}"),
                        "server_error",
                        None,
                    )),
                )
            })?;
            let _ = tx
                .send(super::PeerProxyRequest {
                    peer_id: *peer_id,
                    body,
                    response_tx: resp_tx,
                })
                .await;
            let peer_resp = resp_rx.await.map_err(|_| {
                (
                    Status::BadGateway,
                    Json(ErrorResponse::new(
                        "peer request dropped",
                        "server_error",
                        Some("peer_error"),
                    )),
                )
            })?;
            let resp_json = peer_resp.map_err(|e| {
                (
                    Status::BadGateway,
                    Json(ErrorResponse::new(&e, "server_error", Some("peer_error"))),
                )
            })?;
            let response: ChatCompletionResponse =
                serde_json::from_value(resp_json).map_err(|e| {
                    (
                        Status::BadGateway,
                        Json(ErrorResponse::new(
                            &format!("peer response parse error: {e}"),
                            "server_error",
                            None,
                        )),
                    )
                })?;

            let (input_tokens, output_tokens) = match &response.usage {
                Some(u) => (u.prompt_tokens, u.completion_tokens),
                None => (0, 0),
            };
            let elapsed = start.elapsed();
            state.usage_tracker.record(UsageEvent {
                ts: chrono::Utc::now(),
                key_hash: auth.key_hash,
                key_name: auth.key_name,
                model,
                input_tokens,
                output_tokens,
                latency_ms: elapsed.as_millis() as u64,
                backend: backend_name,
            });

            return Ok(Either::Left(Json(response)));
        }
    }

    if is_stream {
        let resp = backend::proxy_chat_completion_stream(&state.client, &resolved, &request)
            .await
            .map_err(|e| proxy_error_to_status(e))?;

        let state_clone = Arc::clone(state.inner());
        let key_hash = auth.key_hash.clone();
        let key_name = auth.key_name.clone();

        let stream = EventStream! {
            use futures_util::StreamExt;
            let mut byte_stream = resp.bytes_stream();
            let mut buf = String::new();
            let mut last_usage: Option<Usage> = None;

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!("stream chunk error: {e}");
                        break;
                    }
                };

                buf.push_str(&String::from_utf8_lossy(&bytes));

                // SSE protocol: events are separated by double newlines
                while let Some(pos) = buf.find("\n\n") {
                    let event_text = buf[..pos].to_string();
                    buf = buf[pos + 2..].to_string();

                    for line in event_text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data.trim() == "[DONE]" {
                                yield Event::data("[DONE]");
                            } else if let Ok(chunk) =
                                serde_json::from_str::<ChatCompletionChunk>(data)
                            {
                                if chunk.usage.is_some() {
                                    last_usage = chunk.usage.clone();
                                }
                                yield Event::data(
                                    serde_json::to_string(&chunk).unwrap_or_default(),
                                );
                            } else {
                                // Pass through unparseable data as-is
                                yield Event::data(data.to_string());
                            }
                        }
                    }
                }
            }

            // Record usage after stream completes
            let (input_tokens, output_tokens) = match last_usage {
                Some(u) => (u.prompt_tokens, u.completion_tokens),
                None => (0, 0),
            };
            let elapsed = start.elapsed();
            state_clone.usage_tracker.record(UsageEvent {
                ts: chrono::Utc::now(),
                key_hash,
                key_name,
                model: model.clone(),
                input_tokens,
                output_tokens,
                latency_ms: elapsed.as_millis() as u64,
                backend: backend_name.clone(),
            });
        };

        Ok(Either::Right(stream))
    } else {
        let response = backend::proxy_chat_completion(&state.client, &resolved, &request)
            .await
            .map_err(|e| proxy_error_to_status(e))?;

        // Record usage
        let (input_tokens, output_tokens) = match &response.usage {
            Some(u) => (u.prompt_tokens, u.completion_tokens),
            None => (0, 0),
        };
        let elapsed = start.elapsed();
        state.usage_tracker.record(UsageEvent {
            ts: chrono::Utc::now(),
            key_hash: auth.key_hash,
            key_name: auth.key_name,
            model,
            input_tokens,
            output_tokens,
            latency_ms: elapsed.as_millis() as u64,
            backend: backend_name,
        });

        Ok(Either::Left(Json(response)))
    }
}

#[get("/v1/models")]
async fn models(
    _auth: AuthedKey,
    state: &State<Arc<AiProxyState>>,
) -> Json<ModelsResponse> {
    let data = backend::list_models(state).await;
    Json(ModelsResponse {
        object: "list".to_string(),
        data,
    })
}

#[get("/v1/usage")]
async fn usage(
    auth: AuthedKey,
    state: &State<Arc<AiProxyState>>,
) -> Json<UsageResponse> {
    let recent = state
        .usage_tracker
        .recent_events(&auth.key_hash, auth.budget_window, 50);

    let tokens_used = state
        .usage_tracker
        .tokens_in_window(&auth.key_hash, auth.budget_window);

    let tokens_remaining = if auth.token_budget == 0 {
        -1 // unlimited
    } else {
        (auth.token_budget - tokens_used).max(0)
    };

    let window_str = humantime::format_duration(auth.budget_window).to_string();

    Json(UsageResponse {
        key_name: auth.key_name,
        window: window_str,
        tokens_used,
        tokens_remaining,
        token_budget: auth.token_budget,
        recent_events: recent
            .into_iter()
            .map(|e| UsageEventResponse {
                ts: e.ts.to_rfc3339(),
                model: e.model,
                input_tokens: e.input_tokens,
                output_tokens: e.output_tokens,
                latency_ms: e.latency_ms,
                backend: e.backend,
            })
            .collect(),
    })
}

#[get("/health")]
async fn health(state: &State<Arc<AiProxyState>>) -> (Status, (ContentType, &'static str)) {
    let backends = state.backends.read().await;
    let has_backend = backends.ollama.is_some() || backends.unsloth.is_some();
    if has_backend {
        (Status::Ok, (ContentType::JSON, r#"{"status":"ok"}"#))
    } else {
        (
            Status::ServiceUnavailable,
            (ContentType::JSON, r#"{"status":"no_backends"}"#),
        )
    }
}

fn proxy_error_to_status(err: ProxyError) -> (Status, Json<ErrorResponse>) {
    match err {
        ProxyError::Backend(msg) => (
            Status::BadGateway,
            Json(ErrorResponse::new(msg, "server_error", Some("backend_error"))),
        ),
        ProxyError::BackendStatus(status, body) => {
            let rocket_status =
                Status::from_code(status).unwrap_or(Status::InternalServerError);
            (
                rocket_status,
                Json(ErrorResponse::new(body, "upstream_error", None)),
            )
        }
        ProxyError::NoBackend => (
            Status::BadGateway,
            Json(ErrorResponse::new(
                "no backend available for this model",
                "server_error",
                Some("no_backend"),
            )),
        ),
    }
}

pub fn build_rocket(
    state: Arc<AiProxyState>,
    port: u16,
    host: &str,
) -> rocket::Rocket<rocket::Build> {
    let address: std::net::IpAddr = host
        .parse()
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    let config = rocket::Config {
        port,
        address,
        log_level: rocket::config::LogLevel::Off,
        shutdown: rocket::config::Shutdown {
            ctrlc: false,
            #[cfg(unix)]
            signals: std::collections::HashSet::new(),
            ..Default::default()
        },
        ..rocket::Config::default()
    };

    rocket::custom(config)
        .manage(state)
        .mount(
            "/",
            routes![chat_completions, models, usage, health],
        )
}
