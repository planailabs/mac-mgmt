pub mod agent;
pub mod connector;
pub mod relay_client;
pub mod session;
pub mod settings_tools;
pub mod tools;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub use connector::ConnectorConfig;
pub use session::models::{HealerEvent, HealerMessage, HealerSession, SessionState};

use agent::InstanceInfo;
use mac_mgmt_common::ServiceExtState;
use swiftide::traits::ToolBox as _;

/// Shared state for the healer subsystem. Clone-friendly (inner Arc).
#[derive(Clone)]
pub struct HealerState {
    inner: Arc<HealerStateInner>,
}

struct HealerStateInner {
    pool: PgPool,
    connector_config: ConnectorConfig,
    running: DashMap<Uuid, RunningSession>,
    shutting_down: AtomicBool,
    push_fn: Option<tools::PushFn>,
}

struct RunningSession {
    cancel: CancellationToken,
    pause_requested: Arc<AtomicBool>,
    events_tx: broadcast::Sender<HealerEvent>,
    /// Currently executing tools (in-memory only, not persisted).
    running_tools: Arc<std::sync::Mutex<Vec<session::RunningTool>>>,
}

/// Request to spawn a new healer session.
pub struct SpawnRequest {
    pub cluster_id: Uuid,
    pub instance_id: String,
    pub created_by: String,
    pub user_message: Option<String>,
    pub relay_url: String,
    pub services_extended: Vec<ServiceExtState>,
    pub sample: Option<serde_json::Value>,
    pub file_tunnels: serde_json::Value,
    pub shell_tunnels: serde_json::Value,
    pub cluster_instances: Vec<InstanceInfo>,
    pub cluster_name: String,
    pub hostname: String,
    /// Skip the 1-hour cooldown (set when DEV_ONLY_NO_AUTH=1).
    pub skip_cooldown: bool,
}

impl HealerState {
    pub fn new(pool: PgPool, connector_config: ConnectorConfig) -> Self {
        Self {
            inner: Arc::new(HealerStateInner {
                pool,
                connector_config,
                running: DashMap::new(),
                shutting_down: AtomicBool::new(false),
                push_fn: None,
            }),
        }
    }

    /// Set the push callback for sending SSE events to daemons.
    /// Must be called after construction, before spawning sessions.
    pub fn set_push_fn(&mut self, f: tools::PushFn) {
        Arc::get_mut(&mut self.inner)
            .expect("set_push_fn must be called before cloning HealerState")
            .push_fn = Some(f);
    }

    /// Spawn a new healer session. Returns the session ID immediately.
    pub async fn spawn_session(&self, req: SpawnRequest) -> Result<Uuid> {
        // Guard: no concurrent sessions for the same instance
        let has_running = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM healer_sessions \
             WHERE instance_id = $1 AND state NOT IN \
             ('completed', 'done', 'failed', 'cancelled', 'needs_human_attention'))",
        )
        .bind(&req.instance_id)
        .fetch_one(&self.inner.pool)
        .await
        .unwrap_or(false);

        if has_running {
            anyhow::bail!("a healer session is already running for this instance");
        }

        // Guard: 1-hour cooldown between sessions (skip in dev mode)
        if !req.skip_cooldown {
            let recent = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM healer_sessions \
                 WHERE instance_id = $1 AND created_at > now() - interval '1 hour')",
            )
            .bind(&req.instance_id)
            .fetch_one(&self.inner.pool)
            .await
            .unwrap_or(false);

            if recent {
                anyhow::bail!(
                    "a healer session was created for this instance in the last hour — \
                     please wait before starting another"
                );
            }
        }

        // 1. Build initial issues snapshot
        let initial_issues =
            serde_json::to_value(&req.services_extended).unwrap_or_else(|_| json!([]));

        let state_data = json!({
            "relay_url": req.relay_url,
            "instance_id": req.instance_id,
            "cluster_name": req.cluster_name,
            "hostname": req.hostname,
            "file_tunnels": req.file_tunnels,
            "shell_tunnels": req.shell_tunnels,
            "sample": req.sample,
        });

        // 2. Create session row
        let session_id = session::store::create_session(
            &self.inner.pool,
            req.cluster_id,
            &req.instance_id,
            &req.created_by,
            &initial_issues,
            &state_data,
        )
        .await
        .context("failed to create healer session")?;

        // 3. Transition to Initializing
        session::store::transition_state(
            &self.inner.pool,
            session_id,
            &SessionState::Initializing,
            &state_data,
        )
        .await?;

        // 4. Mint proxy token via internal PgPool
        let (proxy_token, proxy_expires) =
            mint_proxy_token(&self.inner.pool, req.cluster_id, None).await?;

        // 5. Set up cancellation and event broadcasting
        let cancel = CancellationToken::new();
        let pause_requested = Arc::new(AtomicBool::new(false));
        let (events_tx, _) = broadcast::channel::<HealerEvent>(4096);
        let running_tools = Arc::new(std::sync::Mutex::new(Vec::new()));
        self.inner.running.insert(
            session_id,
            RunningSession {
                cancel: cancel.clone(),
                pause_requested: pause_requested.clone(),
                events_tx: events_tx.clone(),
                running_tools: running_tools.clone(),
            },
        );

        // 6. Spawn the agent task
        let state = self.clone();
        let connector_config = self.inner.connector_config.clone();
        tokio::spawn(async move {
            let result = run_agent_session(
                &state,
                session_id,
                req,
                proxy_token,
                proxy_expires,
                cancel.clone(),
                pause_requested,
                events_tx.clone(),
                running_tools,
                connector_config,
                None, // no restored history
            )
            .await;

            // Clean up
            state.inner.running.remove(&session_id);

            if let Err(e) = &result {
                tracing::error!(session_id = %session_id, err = %e, "healer session failed");
                let _ = session::store::fail_session(
                    &state.inner.pool,
                    session_id,
                    &e.to_string(),
                    &json!({}),
                )
                .await;
            }

            // Broadcast done event
            let final_state = session::store::get_session(&state.inner.pool, session_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.state.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let _ = events_tx.send(HealerEvent::Done { state: final_state });
        });

        Ok(session_id)
    }

    /// Resume all interrupted sessions on server startup.
    pub async fn resume_interrupted(&self) -> Result<usize> {
        let sessions = session::store::find_resumable(&self.inner.pool).await?;
        let count = sessions.len();

        for sess in sessions {
            let sid = sess.id;
            tracing::info!(
                session_id = %sid,
                state = sess.state.as_str(),
                instance = %sess.instance_id,
                "resuming interrupted healer session"
            );

            if let Err(e) = self.resume_session_internal(sess).await {
                tracing::error!(session_id = %sid, err = %e, "failed to resume session");
            }
        }

        Ok(count)
    }

    /// Resume a paused session (manual resume via API).
    pub async fn resume_session(&self, session_id: Uuid) -> Result<()> {
        let sess = session::store::get_session(&self.inner.pool, session_id)
            .await?
            .context("session not found")?;

        if sess.state != SessionState::Paused {
            anyhow::bail!(
                "session {} is in state {:?}, can only resume from Paused",
                session_id,
                sess.state
            );
        }

        self.resume_session_internal(sess).await
    }

    async fn resume_session_internal(&self, sess: HealerSession) -> Result<()> {
        let session_id = sess.id;

        // Load messages for history restoration
        let messages = session::store::get_messages(&self.inner.pool, session_id).await?;

        // Extract context from state_data
        let mut relay_url = sess.state_data["relay_url"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        // If relay_url is missing from state_data, try the current heartbeat
        if relay_url.is_empty() {
            relay_url = sqlx::query_scalar(
                "SELECT relay_proxy_url FROM daemon_heartbeats \
                 WHERE instance_id = $1 AND relay_proxy_url IS NOT NULL AND relay_proxy_url != '' \
                 LIMIT 1",
            )
            .bind(&sess.instance_id)
            .fetch_optional(&self.inner.pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
            if relay_url.is_empty() {
                anyhow::bail!(
                    "session {} has no relay_url and daemon has no relay_proxy_url — \
                     cannot resume without relay connectivity",
                    session_id
                );
            }
            tracing::info!("relay_url missing from state_data, using heartbeat value: {relay_url}");
        }
        let instance_id = sess.instance_id.clone();
        let cluster_name = sess.state_data["cluster_name"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let hostname = sess.state_data["hostname"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let file_tunnels = sess.state_data["file_tunnels"].clone();
        let shell_tunnels = sess.state_data["shell_tunnels"].clone();
        let sample = sess.state_data.get("sample").cloned();

        // Mint fresh proxy token
        let (proxy_token, proxy_expires) =
            mint_proxy_token(&self.inner.pool, sess.cluster_id, None).await?;

        // Transition back to a running state
        let resume_state = match sess
            .state_data
            .get("last_state")
            .and_then(|v| v.as_str())
            .and_then(SessionState::from_str)
        {
            Some(s) if s.is_active() => s,
            _ => SessionState::Diagnosing,
        };
        session::store::transition_state(
            &self.inner.pool,
            session_id,
            &resume_state,
            &sess.state_data,
        )
        .await?;

        // Set up cancellation and events
        let cancel = CancellationToken::new();
        let pause_requested = Arc::new(AtomicBool::new(false));
        let (events_tx, _) = broadcast::channel::<HealerEvent>(4096);
        let running_tools = Arc::new(std::sync::Mutex::new(Vec::new()));
        self.inner.running.insert(
            session_id,
            RunningSession {
                cancel: cancel.clone(),
                pause_requested: pause_requested.clone(),
                events_tx: events_tx.clone(),
                running_tools: running_tools.clone(),
            },
        );

        // Build a SpawnRequest from saved state
        let req = SpawnRequest {
            cluster_id: sess.cluster_id,
            instance_id,
            created_by: sess.created_by,
            user_message: None,
            relay_url,
            services_extended: serde_json::from_value(sess.initial_issues.clone())
                .unwrap_or_default(),
            sample,
            file_tunnels,
            shell_tunnels,
            cluster_instances: Vec::new(),
            cluster_name,
            hostname,
            skip_cooldown: true, // resuming — cooldown doesn't apply
        };

        let state = self.clone();
        let connector_config = self.inner.connector_config.clone();
        let restored_history = Some(messages);

        tokio::spawn(async move {
            let result = run_agent_session(
                &state,
                session_id,
                req,
                proxy_token,
                proxy_expires,
                cancel,
                pause_requested,
                events_tx.clone(),
                running_tools,
                connector_config,
                restored_history,
            )
            .await;

            state.inner.running.remove(&session_id);

            if let Err(e) = &result {
                tracing::error!(session_id = %session_id, err = %e, "resumed healer session failed");
                let _ = session::store::fail_session(
                    &state.inner.pool,
                    session_id,
                    &e.to_string(),
                    &json!({}),
                )
                .await;
            }

            let final_state = session::store::get_session(&state.inner.pool, session_id)
                .await
                .ok()
                .flatten()
                .map(|s| s.state.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let _ = events_tx.send(HealerEvent::Done { state: final_state });
        });

        Ok(())
    }

    /// Request a running session to pause at its next checkpoint.
    /// The agent will finish its current LLM request, then transition to Paused.
    pub fn pause_session(&self, session_id: Uuid) -> Result<()> {
        if let Some(entry) = self.inner.running.get(&session_id) {
            entry.pause_requested.store(true, Ordering::Relaxed);
            Ok(())
        } else {
            Err(anyhow::anyhow!("session is not running"))
        }
    }

    /// Cancel a running session.
    pub async fn cancel_session(&self, session_id: Uuid) -> Result<()> {
        if let Some(entry) = self.inner.running.get(&session_id) {
            entry.cancel.cancel();
            Ok(())
        } else {
            // Session might not be running — mark it cancelled in DB directly
            session::store::transition_state(
                &self.inner.pool,
                session_id,
                &SessionState::Cancelled,
                &json!({}),
            )
            .await
        }
    }

    /// List sessions for a cluster.
    pub async fn list_sessions(&self, cluster_id: Uuid) -> Result<Vec<HealerSession>> {
        session::store::list_sessions(&self.inner.pool, cluster_id).await
    }

    /// Get a session with its messages.
    pub async fn get_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<(HealerSession, Vec<HealerMessage>)>> {
        let Some(sess) = session::store::get_session(&self.inner.pool, session_id).await? else {
            return Ok(None);
        };
        let msgs = session::store::get_messages(&self.inner.pool, session_id).await?;
        Ok(Some((sess, msgs)))
    }

    /// Subscribe to live events for a running session.
    pub fn subscribe(&self, session_id: Uuid) -> Option<broadcast::Receiver<HealerEvent>> {
        self.inner
            .running
            .get(&session_id)
            .map(|entry| entry.events_tx.subscribe())
    }

    /// Get the current running tools snapshot for a session.
    pub fn running_tools(&self, session_id: Uuid) -> Vec<session::RunningTool> {
        self.inner
            .running
            .get(&session_id)
            .map(|entry| entry.running_tools.lock().unwrap().clone())
            .unwrap_or_default()
    }

    /// Graceful shutdown: signal all sessions to stop at the next safe point,
    /// wait for them to checkpoint, with a timeout.
    pub async fn graceful_shutdown(&self, timeout: std::time::Duration) -> usize {
        self.inner.shutting_down.store(true, Ordering::SeqCst);

        let deadline = tokio::time::Instant::now() + timeout;
        let initial_count = self.inner.running.len();

        // Wait for all sessions to drain
        loop {
            if self.inner.running.is_empty() {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                tracing::warn!(
                    remaining = self.inner.running.len(),
                    "healer shutdown timed out, some sessions may not be cleanly checkpointed"
                );
                // Force-cancel remaining sessions
                for entry in self.inner.running.iter() {
                    entry.cancel.cancel();
                }
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        initial_count
    }

    /// Check if the system is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        self.inner.shutting_down.load(Ordering::SeqCst)
    }
}

/// Mint a proxy token directly via PgPool.
async fn mint_proxy_token(
    pool: &PgPool,
    cluster_id: Uuid,
    organization_id: Option<Uuid>,
) -> Result<(String, DateTime<Utc>)> {
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let raw_token = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = Utc::now() + chrono::Duration::hours(6);

    sqlx::query(
        "INSERT INTO tokens (cluster_id, organization_id, token_hash, label, kind, expires_at) \
         VALUES ($1, $2, $3, 'healer', 'proxy', $4)",
    )
    .bind(cluster_id)
    .bind(organization_id)
    .bind(&hash)
    .bind(expires_at)
    .execute(pool)
    .await
    .context("failed to mint proxy token")?;

    Ok((raw_token, expires_at))
}

/// Run the agent session. This is the main async task that drives the LLM agent.
async fn run_agent_session(
    state: &HealerState,
    session_id: Uuid,
    mut req: SpawnRequest,
    proxy_token: String,
    proxy_expires: DateTime<Utc>,
    cancel: CancellationToken,
    pause_requested: Arc<AtomicBool>,
    events_tx: broadcast::Sender<HealerEvent>,
    running_tools: Arc<std::sync::Mutex<Vec<session::RunningTool>>>,
    connector_config: ConnectorConfig,
    restored_history: Option<Vec<HealerMessage>>,
) -> Result<()> {
    let pool = &state.inner.pool;

    // 1. Resolve LLM
    let llm = connector::resolve_llm(&connector_config)
        .await
        .context("failed to resolve LLM")?;

    // 2. Build relay client (with event broadcasting for connectivity status)
    //    If relay_url is empty, poll heartbeats until it appears (the daemon
    //    may not have received its proxy_url from the relay yet).
    if req.relay_url.is_empty() {
        tracing::warn!("relay_url is empty, waiting for daemon to report one via heartbeat");
        let _ = events_tx.send(session::HealerEvent::Status {
            message: "Waiting for daemon to connect to relay...".to_string(),
        });
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let url: Option<String> = sqlx::query_scalar(
                "SELECT relay_proxy_url FROM daemon_heartbeats \
                 WHERE instance_id = $1 AND relay_proxy_url IS NOT NULL AND relay_proxy_url != '' \
                 LIMIT 1",
            )
            .bind(&req.instance_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            if let Some(url) = url {
                tracing::info!("relay_url appeared: {url}");
                req.relay_url = url;
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!(
                    "daemon did not report a relay proxy URL within 2 minutes — \
                     check that the relay has proxy_url configured"
                );
            }
        }
    }
    let relay_client = Arc::new(
        relay_client::RelayClient::new(req.relay_url.clone(), proxy_token)
            .with_events(events_tx.clone())
            .with_heartbeat_ctx(pool.clone(), req.instance_id.clone()),
    );

    // 3. Extract tunnel names
    let file_tunnel_names: Vec<String> = req
        .file_tunnels
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let shell_command_names: Vec<String> = req
        .shell_tunnels
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let cluster_instance_prefixes: Vec<String> = req
        .cluster_instances
        .iter()
        .map(|i| i.instance_prefix.clone())
        .collect();

    // 4. Create native swiftide tools (no MCP transport needed)
    let tool_ctx = tools::ToolContext {
        relay: relay_client,
        target_instance: req.instance_id.chars().take(12).collect(),
        cluster_instances: cluster_instance_prefixes,
        file_tunnels: file_tunnel_names.clone(),
        file_tunnels_full: req.file_tunnels.clone(),
        shell_commands: shell_command_names.clone(),
        shell_commands_full: req.shell_tunnels.clone(),
        pool: pool.clone(),
        session_id,
        cluster_id: req.cluster_id,
        instance_id: req.instance_id.clone(),
        push_fn: state.inner.push_fn.clone(),
        events_tx: events_tx.clone(),
    };
    let healer_tools = tools::all_tools(tool_ctx.clone());
    let settings_tools = settings_tools::all_settings_tools(tool_ctx);

    // 6. Build system prompt
    let sample_summary = req
        .sample
        .as_ref()
        .map(|s| agent::format_sample_summary(s))
        .unwrap_or_default();

    let resume_context = if restored_history.is_some() {
        Some(
            "This session has been resumed from a previous checkpoint. The conversation history above contains your previous work. Continue from where you left off.",
        )
    } else {
        None
    };

    let system_prompt = agent::build_system_prompt(
        &req.cluster_name,
        &req.cluster_id.to_string(),
        &req.instance_id,
        &req.hostname,
        &req.cluster_instances,
        &req.services_extended,
        &sample_summary,
        &file_tunnel_names,
        &shell_command_names,
        resume_context,
    );

    // 7. Transition to Diagnosing
    let transition = |new_state: &SessionState, data: &serde_json::Value| {
        let pool = pool.clone();
        let events_tx = events_tx.clone();
        let state_str = new_state.as_str().to_string();
        let data = data.clone();
        let new_state = new_state.clone();
        async move {
            session::store::transition_state(&pool, session_id, &new_state, &data)
                .await
                .ok();
            let _ = events_tx.send(HealerEvent::State {
                state: state_str,
                state_data: data,
            });
        }
    };

    transition(&SessionState::Diagnosing, &json!({})).await;

    // 8. Build and run agent
    let pool_msg = pool.clone();
    let events_tx_msg = events_tx.clone();
    let budget_limit = connector_config.token_budget;
    let token_usage = llm.token_usage.clone();
    let is_cloud = llm.is_cloud;
    // state is Clone (inner Arc) — we can pass it into closures for shutdown checks
    let pool_hook = pool.clone();
    let events_tx_hook = events_tx.clone();

    // Determine initial prompt
    let initial_prompt = if restored_history.is_some() {
        "You have been resumed. Review your previous conversation above and continue your work from where you left off.".to_string()
    } else {
        let msg = req.user_message.unwrap_or_else(|| {
            "Diagnose and fix the detected issues. Start by listing available tools and reading logs.".to_string()
        });
        // Persist the initial user message so it appears in the chat log
        session::store::append_message(pool, session_id, "user", &msg, None)
            .await
            .ok();
        let _ = events_tx.send(session::HealerEvent::Message {
            role: "user".to_string(),
            content: msg.clone(),
            metadata: None,
            created_at: chrono::Utc::now(),
        });
        msg
    };

    // Build agent with swiftide
    let mut agent = {
        use swiftide::agents;

        let mut builder = agents::Agent::builder();

        // Set LLM provider
        match &llm.provider {
            connector::LlmProvider::Ollama(o) => {
                builder.llm(o);
            }
            connector::LlmProvider::Anthropic(a) => {
                builder.llm(a);
            }
        }

        // Extract role/content from ChatMessage enum for persistence
        /// Extract role/content from ChatMessage. Returns None for ToolOutput
        /// (handled by before_tool/after_tool hooks to avoid duplicates).
        fn extract_role_content(
            msg: &swiftide::chat_completion::ChatMessage,
        ) -> Option<(String, String)> {
            match msg {
                swiftide::chat_completion::ChatMessage::System(s) => {
                    Some(("system".to_string(), s.clone()))
                }
                swiftide::chat_completion::ChatMessage::User(s) => {
                    Some(("user".to_string(), s.clone()))
                }
                swiftide::chat_completion::ChatMessage::Assistant(s, tool_calls) => {
                    let mut content = s.clone().unwrap_or_default();
                    // Include tool call names in the assistant message for visibility
                    if let Some(calls) = tool_calls {
                        if !calls.is_empty() && content.is_empty() {
                            content = calls
                                .iter()
                                .map(|tc| format!("`{}`", tc.name()))
                                .collect::<Vec<_>>()
                                .join(", ");
                        }
                    }
                    Some(("assistant".to_string(), content))
                }
                // Skip ToolOutput — persisted by after_tool hook
                swiftide::chat_completion::ChatMessage::ToolOutput(_, _) => None,
                swiftide::chat_completion::ChatMessage::Summary(s) => {
                    Some(("summary".to_string(), s.clone()))
                }
            }
        }

        for tool in healer_tools {
            builder.add_tool(tool);
        }
        for tool in settings_tools {
            builder.add_tool(tool);
        }

        // Connect to Context7 MCP server if API key is configured.
        // Wrap MCP tools with sanitized names — Anthropic rejects colons
        // but swiftide formats MCP tool names as "server:tool".
        let mut _mcp_toolbox = None;
        if let Some(api_key) = &connector_config.context7_api_key {
            match connect_context7(api_key).await {
                Ok(toolbox) => {
                    tracing::info!("Context7 MCP connected");
                    match toolbox.available_tools().await {
                        Ok(tools) => {
                            for tool in tools {
                                builder.add_tool(RenamedTool::wrap(tool));
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Context7: failed to list tools: {e:#}");
                        }
                    }
                    _mcp_toolbox = Some(toolbox);
                }
                Err(e) => {
                    tracing::warn!("Context7 MCP connection failed: {e:#}");
                }
            }
        }

        let events_tx_before_tool = events_tx.clone();
        let running_tools_before = running_tools.clone();
        let pool_after_tool = pool.clone();
        let events_tx_after_tool = events_tx.clone();
        let running_tools_after = running_tools.clone();

        let state_ref = state.clone();
        builder
            .system_prompt(system_prompt)
            .on_new_message(move |_agent, msg| {
                let pool = pool_msg.clone();
                let events_tx = events_tx_msg.clone();
                let extracted = extract_role_content(msg);
                Box::pin(async move {
                    let Some((role, content)) = extracted else {
                        return Ok(()); // ToolOutput handled by after_tool
                    };
                    if content.is_empty() {
                        return Ok(());
                    }
                    // Persist message
                    session::store::append_message(&pool, session_id, &role, &content, None)
                        .await
                        .ok();

                    // Broadcast to SSE subscribers
                    let _ = events_tx.send(HealerEvent::Message {
                        role,
                        content,
                        metadata: None,
                        created_at: Utc::now(),
                    });
                    Ok(())
                })
            })
            .before_tool(move |_agent, tool_call| {
                let events_tx = events_tx_before_tool.clone();
                let running_tools = running_tools_before.clone();
                let name = tool_call.name().to_string();
                let args = tool_call.args().map(String::from);
                Box::pin(async move {
                    // Add to in-memory running tools and broadcast snapshot
                    let tool = session::RunningTool {
                        name: name.clone(),
                        args: args.clone(),
                        started_at: Utc::now(),
                    };
                    let snapshot = {
                        let mut tools = running_tools.lock().unwrap();
                        tools.push(tool);
                        tools.clone()
                    };
                    let _ = events_tx.send(HealerEvent::RunningTools { tools: snapshot });
                    Ok(())
                })
            })
            .after_tool(move |_agent, tool_call, result| {
                let pool = pool_after_tool.clone();
                let events_tx = events_tx_after_tool.clone();
                let running_tools = running_tools_after.clone();
                let name = tool_call.name().to_string();
                let args = tool_call.args().map(String::from);
                let (status, output) = match result {
                    Ok(out) => ("ok", out.to_string()),
                    Err(e) => ("error", e.to_string()),
                };
                Box::pin(async move {
                    // Remove from running tools and broadcast snapshot
                    let snapshot = {
                        let mut tools = running_tools.lock().unwrap();
                        tools.retain(|t| t.name != name);
                        tools.clone()
                    };
                    let _ = events_tx.send(HealerEvent::RunningTools { tools: snapshot });

                    // Persist and broadcast the result
                    let content = format!("{name}: {output}");
                    let metadata = serde_json::json!({
                        "tool_name": name,
                        "tool_args": args.as_deref().unwrap_or("{}"),
                        "status": status,
                    });
                    session::store::append_message(
                        &pool,
                        session_id,
                        "tool_result",
                        &content,
                        Some(&metadata),
                    )
                    .await
                    .ok();
                    let _ = events_tx.send(HealerEvent::Message {
                        role: "tool_result".to_string(),
                        content,
                        metadata: Some(metadata),
                        created_at: Utc::now(),
                    });
                    Ok(())
                })
            })
            .before_completion(move |_agent, _req| {
                let cancel = cancel.clone();
                let pause_requested = pause_requested.clone();
                let pool = pool_hook.clone();
                let events_tx = events_tx_hook.clone();
                let token_usage = token_usage.clone();
                let state_ref = state_ref.clone();
                Box::pin(async move {
                    // 1. Check shutdown
                    if state_ref.is_shutting_down() {
                        session::store::transition_state(
                            &pool,
                            session_id,
                            &SessionState::AwaitingRetry,
                            &json!({"reason": "server_shutdown"}),
                        )
                        .await
                        .ok();
                        let _ = events_tx.send(HealerEvent::State {
                            state: "awaiting_retry".to_string(),
                            state_data: json!({"reason": "server_shutdown"}),
                        });
                        return Err(anyhow::anyhow!("server shutting down"));
                    }

                    // 2. Check cancellation
                    if cancel.is_cancelled() {
                        session::store::transition_state(
                            &pool,
                            session_id,
                            &SessionState::Cancelled,
                            &json!({}),
                        )
                        .await
                        .ok();
                        let _ = events_tx.send(HealerEvent::State {
                            state: "cancelled".to_string(),
                            state_data: json!({}),
                        });
                        return Err(anyhow::anyhow!("session cancelled"));
                    }

                    // 2b. Check manual pause request
                    if pause_requested.swap(false, Ordering::Relaxed) {
                        let data = json!({"reason": "manual_pause"});
                        session::store::transition_state(
                            &pool,
                            session_id,
                            &SessionState::Paused,
                            &data,
                        )
                        .await
                        .ok();
                        let _ = events_tx.send(HealerEvent::State {
                            state: "paused".to_string(),
                            state_data: data,
                        });
                        return Err(anyhow::anyhow!("session paused by user"));
                    }

                    // 3. Check proxy token expiry
                    if Utc::now() + chrono::Duration::minutes(30) > proxy_expires {
                        session::store::transition_state(
                            &pool,
                            session_id,
                            &SessionState::AwaitingRetry,
                            &json!({"reason": "proxy_token_expiring"}),
                        )
                        .await
                        .ok();
                        let _ = events_tx.send(HealerEvent::State {
                            state: "awaiting_retry".to_string(),
                            state_data: json!({"reason": "proxy_token_expiring"}),
                        });
                        return Err(anyhow::anyhow!("proxy token expiring"));
                    }

                    // 4. Check cloud token budget
                    if is_cloud && budget_limit > 0 {
                        let used = token_usage.load(Ordering::Relaxed);
                        if used >= budget_limit {
                            let data = json!({
                                "reason": "token_budget_exceeded",
                                "tokens_used": used,
                                "budget_limit": budget_limit
                            });
                            session::store::transition_state(
                                &pool,
                                session_id,
                                &SessionState::Paused,
                                &data,
                            )
                            .await
                            .ok();
                            let _ = events_tx.send(HealerEvent::State {
                                state: "paused".to_string(),
                                state_data: data,
                            });
                            return Err(anyhow::anyhow!(
                                "token budget exceeded ({used}/{budget_limit})"
                            ));
                        }
                    }

                    Ok(())
                })
            })
            .limit(50);

        builder.build().context("failed to build agent")?
    };

    // Run the agent
    agent
        .query(initial_prompt)
        .await
        .context("agent query failed")?;

    // Mark completed
    session::store::transition_state(
        pool,
        session_id,
        &SessionState::Completed,
        &json!({"summary": "agent finished"}),
    )
    .await?;

    let _ = events_tx.send(HealerEvent::State {
        state: "completed".to_string(),
        state_data: json!({"summary": "agent finished"}),
    });

    Ok(())
}

/// Wrapper that sanitizes tool names for Anthropic compatibility.
/// Replaces colons with hyphens (e.g. "Context7:query-docs" → "context7-query-docs").
#[derive(Clone)]
struct RenamedTool {
    inner: Box<dyn swiftide::chat_completion::Tool>,
    name: String,
    spec: swiftide::chat_completion::ToolSpec,
}

impl RenamedTool {
    fn wrap(tool: Box<dyn swiftide::chat_completion::Tool>) -> Box<dyn swiftide::chat_completion::Tool> {
        let orig_name = tool.name().to_string();
        let sanitized = orig_name
            .replace(':', "-")
            .replace(' ', "_")
            .to_lowercase();
        let mut spec = tool.tool_spec();
        spec.name = sanitized.clone();
        Box::new(Self {
            inner: tool,
            name: sanitized,
            spec,
        })
    }
}

#[async_trait::async_trait]
impl swiftide::chat_completion::Tool for RenamedTool {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(&self.name)
    }

    fn tool_spec(&self) -> swiftide::chat_completion::ToolSpec {
        self.spec.clone()
    }

    async fn invoke(
        &self,
        agent_context: &dyn swiftide::traits::AgentContext,
        tool_call: &swiftide::chat_completion::ToolCall,
    ) -> Result<swiftide::chat_completion::ToolOutput, swiftide::chat_completion::errors::ToolError> {
        self.inner.invoke(agent_context, tool_call).await
    }
}

/// Connect to the Context7 documentation MCP server via SSE.
async fn connect_context7(
    api_key: &str,
) -> Result<swiftide::agents::tools::mcp::McpToolbox> {
    let url = format!("https://mcp.context7.com/mcp?api_key={api_key}");
    let worker = rmcp::transport::streamable_http_client::StreamableHttpClientWorker::<reqwest::Client>::new_simple(url);
    let mut toolbox =
        swiftide::agents::tools::mcp::McpToolbox::try_from_transport(worker)
            .await
            .context("Context7 MCP handshake failed")?;
    toolbox.with_name("Context7");
    Ok(toolbox)
}
