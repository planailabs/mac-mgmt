//! mac-mgmt's [`plan_ai_chat_ui::backend::ChatBackend`] implementation: the
//! bridge between the reusable chat sidebar's server functions and this
//! server's chat service (`crate::chat`). Registered once at startup when
//! chat is enabled.

use async_trait::async_trait;
use plan_ai_chat_ui::backend::{BackendResult, ChatBackend};
use plan_ai_chat_ui::render::ModelEntry;
use plan_ai_chat_ui::sidebar::{ChatContext, ChatSessionMeta, ChatSessionSummary};

use crate::chat::{ChatState, ChatUserCtx};
use crate::web::user::{current_user, principal_from};

pub struct MacMgmtChatBackend;

fn chat_state() -> BackendResult<ChatState> {
    crate::server_state::chat_state().ok_or_else(|| "chat is not enabled".to_string())
}

async fn chat_user() -> BackendResult<ChatUserCtx> {
    let user = current_user().await.map_err(|e| e.to_string())?;
    Ok(ChatUserCtx {
        email: user.email.clone(),
        is_admin: user.is_admin,
        principal: principal_from(&user),
    })
}

fn parse_id(id: &str) -> BackendResult<uuid::Uuid> {
    id.parse().map_err(|_| "invalid id".to_string())
}

#[async_trait]
impl ChatBackend for MacMgmtChatBackend {
    async fn context(&self) -> BackendResult<ChatContext> {
        // Must not fail for anonymous/handshake states: enabled=false hides
        // the sidebar.
        if current_user().await.is_err() {
            return Ok(ChatContext {
                enabled: false,
                models: Vec::new(),
            });
        }
        let cfg = crate::config::config();
        let enabled = cfg.chat.enabled && crate::server_state::chat_state().is_some();
        Ok(ChatContext {
            enabled,
            models: cfg
                .model_catalog()
                .models_for("chat")
                .into_iter()
                .map(|m| ModelEntry {
                    name: m.name.clone(),
                    model: m.model.clone(),
                    provider: m.provider.clone(),
                })
                .collect(),
        })
    }

    async fn list_sessions(&self) -> BackendResult<Vec<ChatSessionSummary>> {
        let user = chat_user().await?;
        let chat = chat_state()?;
        let sessions = chat
            .list_sessions(&user.email)
            .await
            .map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(sessions.len());
        for s in sessions {
            let tokens = chat.store().get_token_usage(s.id).await.unwrap_or(0);
            out.push(ChatSessionSummary {
                id: s.id.to_string(),
                label: s.label,
                state: s.state,
                model: s.model,
                updated_at: s.updated_at,
                tokens_used: tokens,
            });
        }
        Ok(out)
    }

    async fn start_session(
        &self,
        provider: Option<String>,
        model: Option<String>,
        message: String,
        page_context: Option<String>,
    ) -> BackendResult<String> {
        let user = chat_user().await?;
        let id = chat_state()?
            .start_session(&user, provider, model, message, page_context)
            .await
            .map_err(|e| e.to_string())?;
        Ok(id.to_string())
    }

    async fn send_message(
        &self,
        session_id: String,
        message: String,
        page_context: Option<String>,
    ) -> BackendResult<()> {
        let user = chat_user().await?;
        chat_state()?
            .send_message(&user, parse_id(&session_id)?, message, page_context)
            .await
            .map_err(|e| e.to_string())
    }

    async fn approve(
        &self,
        session_id: String,
        approval_id: String,
        decision: String,
        reason: Option<String>,
    ) -> BackendResult<()> {
        let user = chat_user().await?;
        let decision = match decision.as_str() {
            "approve" => plan_ai_chat::ApprovalDecision::Approve,
            "approve_all" => plan_ai_chat::ApprovalDecision::ApproveAllForSession,
            _ => plan_ai_chat::ApprovalDecision::Deny { reason },
        };
        chat_state()?
            .resolve_approval(&user, parse_id(&session_id)?, parse_id(&approval_id)?, decision)
            .await
            .map_err(|e| e.to_string())
    }

    async fn pause(&self, session_id: String) -> BackendResult<()> {
        let user = chat_user().await?;
        chat_state()?
            .pause(&user, parse_id(&session_id)?)
            .await
            .map_err(|e| e.to_string())
    }

    async fn cancel(&self, session_id: String) -> BackendResult<()> {
        let user = chat_user().await?;
        chat_state()?
            .cancel(&user, parse_id(&session_id)?)
            .await
            .map_err(|e| e.to_string())
    }

    async fn extend_budget(&self, session_id: String) -> BackendResult<()> {
        let user = chat_user().await?;
        chat_state()?
            .extend_budget(&user, parse_id(&session_id)?)
            .await
            .map_err(|e| e.to_string())
    }

    async fn set_auto_approve(&self, session_id: String, value: bool) -> BackendResult<()> {
        let user = chat_user().await?;
        chat_state()?
            .set_auto_approve(&user, parse_id(&session_id)?, value)
            .await
            .map_err(|e| e.to_string())
    }

    async fn session_meta(&self, session_id: String) -> BackendResult<ChatSessionMeta> {
        let user = chat_user().await?;
        let chat = chat_state()?;
        let id = parse_id(&session_id)?;
        let sess = chat
            .get_session_checked(&user, id)
            .await
            .map_err(|e| e.to_string())?;
        let tokens_used = chat.store().get_token_usage(id).await.unwrap_or(0);
        let token_budget = chat.store().get_token_budget(id).await.unwrap_or(0);
        Ok(ChatSessionMeta {
            label: sess.label,
            state: sess.state,
            provider: sess.provider,
            model: sess.model,
            tokens_used,
            token_budget,
            auto_approve: chat.auto_approve(id),
        })
    }
}
