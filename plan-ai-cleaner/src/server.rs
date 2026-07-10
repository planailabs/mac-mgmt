use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::state::SharedState;
use crate::types::*;

#[derive(Clone)]
pub struct CleanerServer {
    state: SharedState,
    ollama_host: String,
    ollama_port: u16,
    ollama_model: String,
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

impl CleanerServer {
    pub fn new(
        state: SharedState,
        ollama_host: String,
        ollama_port: u16,
        ollama_model: String,
    ) -> Self {
        Self {
            state,
            ollama_host,
            ollama_port,
            ollama_model,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for CleanerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "PII/secret detection and redaction server. Use cleaner_scan to analyze a \
                 document, review the findings, then cleaner_approve to produce a redacted \
                 version. Use cleaner_rehydrate to restore originals in a cloud LLM response."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tool_router]
impl CleanerServer {
    #[tool(
        name = "cleaner_scan",
        description = "Scan a document for PII and secrets. Returns a summary of detected entities for review. Uses regex patterns for high-confidence detection (emails, phones, API keys, etc.) and optionally a local LLM for contextual detection (person names, company names, internal URLs). Review the results, then call cleaner_approve to finalize."
    )]
    async fn scan(&self, Parameters(params): Parameters<ScanParams>) -> String {
        let session_id = uuid::Uuid::new_v4().to_string();

        // Step 1: Regex-based detection.
        let regex_matches = crate::patterns::scan_text(&params.text);
        let mut detections: Vec<(String, EntityCategory, DetectionSource)> = regex_matches
            .into_iter()
            .map(|m| (m.text, m.category, DetectionSource::Regex))
            .collect();

        // Step 2: LLM-based detection (if enabled).
        if params.use_llm {
            let chunks = crate::chunker::chunk_document(&params.text);
            for chunk in chunks {
                match crate::ollama::detect_entities(
                    &self.ollama_host,
                    self.ollama_port,
                    &self.ollama_model,
                    chunk,
                )
                .await
                {
                    Ok(entities) => {
                        for e in entities {
                            detections.push((e.text, e.category, DetectionSource::Llm));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Ollama NER failed for chunk: {e}");
                        // Continue with regex-only results.
                    }
                }
            }
        }

        // Step 3: Build deduplicated entity list.
        let entities = self.state.build_entities(&session_id, detections).await;

        if entities.is_empty() {
            return "No PII or secrets detected in the document.".to_string();
        }

        // Step 4: Create session manifest.
        let manifest = SessionManifest {
            id: session_id.clone(),
            name: params.session_name,
            created_at: chrono::Utc::now(),
            ttl_seconds: 86400, // Will be overridden by store's TTL on load.
            status: SessionStatus::Scanned,
            original_hash: crate::storage::sha256_hex(&params.text),
            entities: entities.clone(),
            redacted_text: None,
            original_text: params.text,
        };

        if let Err(e) = self.state.store().save(&manifest) {
            return format!("Error saving session: {e}");
        }

        // Step 5: Format summary for the user.
        let mut summary = format!(
            "Session: {session_id}\nDetected {} entities:\n\n",
            entities.len()
        );
        summary.push_str("| Placeholder | Category | Original | Source |\n");
        summary.push_str("|-------------|----------|----------|--------|\n");
        for e in &entities {
            summary.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                e.placeholder,
                e.category,
                truncate(&e.original, 40),
                e.source,
            ));
        }
        summary.push_str(&format!(
            "\nCall cleaner_approve with session_id=\"{session_id}\" to finalize. \
             Use remove_ids to exclude false positives."
        ));

        summary
    }

    #[tool(
        name = "cleaner_approve",
        description = "Approve and finalize redactions for a scanned session. Optionally remove entities by ID (false positives) or add manual redactions. Returns the fully redacted document text."
    )]
    async fn approve(&self, Parameters(params): Parameters<ApproveParams>) -> String {
        let mut manifest = match self.state.store().load(&params.session_id) {
            Ok(m) => m,
            Err(e) => return format!("Error loading session: {e}"),
        };

        // Mark entities as not approved if in remove_ids.
        for entity in &mut manifest.entities {
            if params.remove_ids.contains(&entity.id) {
                entity.approved = false;
            }
        }

        // Add manual redactions.
        for manual in &params.add_redactions {
            if manual.text.is_empty() {
                continue;
            }
            let category = EntityCategory::Custom;
            let id = self
                .state
                .next_placeholder(&params.session_id, category)
                .await;
            let placeholder = format!("[{id}]");
            manifest.entities.push(RedactionEntity {
                id,
                category,
                original: manual.text.clone(),
                placeholder,
                source: DetectionSource::Manual,
                approved: true,
            });
        }

        // Apply redactions to produce the clean text.
        let mut redacted = manifest.original_text.clone();
        // Sort entities by original text length descending to avoid partial
        // replacements (e.g., "John Smith" before "John").
        let mut approved: Vec<_> = manifest.entities.iter().filter(|e| e.approved).collect();
        approved.sort_by(|a, b| b.original.len().cmp(&a.original.len()));

        for entity in &approved {
            redacted = redacted.replace(&entity.original, &entity.placeholder);
        }

        manifest.redacted_text = Some(redacted.clone());
        manifest.status = SessionStatus::Approved;

        if let Err(e) = self.state.store().save(&manifest) {
            return format!("Error saving approved session: {e}");
        }

        redacted
    }

    #[tool(
        name = "cleaner_rehydrate",
        description = "Replace placeholders in text (e.g. a cloud LLM response) with the original values from a session. Only works on approved sessions."
    )]
    async fn rehydrate(&self, Parameters(params): Parameters<RehydrateParams>) -> String {
        let manifest = match self.state.store().load(&params.session_id) {
            Ok(m) => m,
            Err(e) => return format!("Error loading session: {e}"),
        };

        // Enforce the approval gate: originals are only re-introduced once the
        // session has been explicitly approved, not just because per-entity
        // flags default to approved on a freshly-scanned session.
        if manifest.status != crate::types::SessionStatus::Approved {
            return "Error: session is not approved; call cleaner_approve first".to_string();
        }

        let mut text = params.text;
        for entity in &manifest.entities {
            if entity.approved {
                text = text.replace(&entity.placeholder, &entity.original);
            }
        }
        text
    }

    #[tool(
        name = "cleaner_session_list",
        description = "List all active (non-expired) redaction sessions."
    )]
    async fn session_list(&self) -> String {
        let sessions = match self.state.store().list() {
            Ok(s) => s,
            Err(e) => return format!("Error listing sessions: {e}"),
        };

        if sessions.is_empty() {
            return "No active sessions.".to_string();
        }

        let mut out = format!("{} session(s):\n\n", sessions.len());
        out.push_str("| ID | Name | Status | Entities | Created |\n");
        out.push_str("|----|------|--------|----------|---------|\n");
        for s in &sessions {
            out.push_str(&format!(
                "| {} | {} | {:?} | {} | {} |\n",
                &s.id[..8],
                s.name.as_deref().unwrap_or("-"),
                s.status,
                s.entities.len(),
                s.created_at.format("%Y-%m-%d %H:%M"),
            ));
        }
        out
    }

    #[tool(
        name = "cleaner_session_delete",
        description = "Delete a redaction session and its encrypted manifest."
    )]
    async fn session_delete(&self, Parameters(params): Parameters<SessionIdParam>) -> String {
        self.state.clear_session(&params.session_id).await;
        match self.state.store().delete(&params.session_id) {
            Ok(()) => format!("Session {} deleted.", params.session_id),
            Err(e) => format!("Error deleting session: {e}"),
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
