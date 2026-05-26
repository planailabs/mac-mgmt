//! plan-ai-memvault-share-agent — MCP server for cross-cluster share proposal review.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The share agent MCP server.
#[derive(Clone)]
pub struct ShareAgentServer {
    client: Arc<dyn memvault_api::MemvaultClient>,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

impl ShareAgentServer {
    pub fn new(client: Arc<dyn memvault_api::MemvaultClient>) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for ShareAgentServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Cross-cluster memvault share proposal review agent. Use share_inbox_list \
                 to see pending proposals, share_inspect to examine one, and \
                 share_approve/share_reject to decide."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InboxListParams {
    /// Only show pending proposals.
    #[serde(default = "default_true")]
    pub only_pending: bool,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CidParam {
    /// Hex-encoded proposal CID.
    pub proposal_cid: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RejectParam {
    /// Hex-encoded proposal CID.
    pub proposal_cid: String,
    /// Reason for rejection.
    pub reason: String,
}

#[tool_router]
impl ShareAgentServer {
    #[tool(
        name = "share_inbox_list",
        description = "List share proposals received from other clusters"
    )]
    async fn share_inbox_list(&self, Parameters(_params): Parameters<InboxListParams>) -> String {
        let proposals = match self.client.share_inbox().await {
            Ok(p) => p,
            Err(e) => return format!("Error: {e}"),
        };

        if proposals.is_empty() {
            return "No pending share proposals.".to_string();
        }

        let mut output = format!("{} pending proposal(s):\n", proposals.len());
        for cid in &proposals {
            output.push_str(&format!("  - {}\n", hex::encode(cid)));
        }
        output
    }

    #[tool(
        name = "share_inspect",
        description = "Deep dive on one share proposal: signature chain, cluster trust history"
    )]
    async fn share_inspect(&self, Parameters(params): Parameters<CidParam>) -> String {
        if hex::decode(&params.proposal_cid).is_err() {
            return format!("Error: invalid hex CID '{}'", params.proposal_cid);
        }
        format!(
            "Proposal {} — inspection details would appear here.",
            params.proposal_cid
        )
    }

    #[tool(
        name = "share_approve",
        description = "Approve a share proposal, optionally with attenuation"
    )]
    async fn share_approve(&self, Parameters(params): Parameters<CidParam>) -> String {
        let cid = match hex::decode(&params.proposal_cid) {
            Ok(c) => c,
            Err(e) => return format!("Error: bad hex: {e}"),
        };
        match self.client.share_decide(&cid, true, None).await {
            Ok(_) => format!("Approved proposal {}.", params.proposal_cid),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        name = "share_reject",
        description = "Reject a share proposal with a reason"
    )]
    async fn share_reject(&self, Parameters(params): Parameters<RejectParam>) -> String {
        let cid = match hex::decode(&params.proposal_cid) {
            Ok(c) => c,
            Err(e) => return format!("Error: bad hex: {e}"),
        };
        match self
            .client
            .share_decide(&cid, false, Some(&params.reason))
            .await
        {
            Ok(_) => format!(
                "Rejected proposal {}: {}",
                params.proposal_cid, params.reason
            ),
            Err(e) => format!("Error: {e}"),
        }
    }
}
