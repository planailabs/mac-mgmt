mod extractor;
pub mod tokenizer;
pub mod features;
pub mod dataset;

pub use extractor::ExportedSession;

use anyhow::{Context, Result};
use std::path::Path;

/// Run the full export pipeline: DB → JSONL files.
pub async fn run_export(db_url: &str, output_dir: &str, min_messages: usize) -> Result<()> {
    let pool = sqlx::PgPool::connect(db_url)
        .await
        .context("failed to connect to database")?;

    let output = Path::new(output_dir);
    std::fs::create_dir_all(output)?;

    // 1. Extract all terminal sessions with messages
    tracing::info!("extracting sessions from database...");
    let sessions = extractor::extract_all(&pool, min_messages).await?;
    tracing::info!("extracted {} sessions", sessions.len());

    if sessions.is_empty() {
        tracing::warn!("no sessions found matching criteria");
        return Ok(());
    }

    // 2. Build vocabulary from the corpus
    tracing::info!("building vocabulary...");
    let vocab = tokenizer::Vocabulary::build_from_sessions(&sessions);
    tracing::info!("vocabulary size: {}", vocab.size());

    // Save vocabulary
    let vocab_path = output.join("vocab.json");
    let vocab_json = serde_json::to_string_pretty(&vocab)?;
    std::fs::write(&vocab_path, vocab_json)?;
    tracing::info!("saved vocabulary to {}", vocab_path.display());

    // 3. Extract features and save as JSONL
    tracing::info!("extracting features...");
    let tool_selector_path = output.join("tool_selector.jsonl");
    let outcome_path = output.join("outcome.jsonl");
    let sessions_path = output.join("sessions.jsonl");

    let mut tool_samples = Vec::new();
    let mut outcome_samples = Vec::new();

    for session in &sessions {
        // Full session export
        let session_json = serde_json::to_string(session)?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&sessions_path)?;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&sessions_path)?;
        writeln!(f, "{session_json}")?;

        // Tool selector samples (one per tool call in the session)
        let ts = features::extract_tool_selector_samples(session, &vocab);
        tool_samples.extend(ts);

        // Outcome sample (one per session)
        if let Some(os) = features::extract_outcome_sample(session, &vocab) {
            outcome_samples.push(os);
        }
    }

    // Write tool selector JSONL
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tool_selector_path)?;
        for sample in &tool_samples {
            writeln!(f, "{}", serde_json::to_string(sample)?)?;
        }
        tracing::info!(
            "wrote {} tool selector samples to {}",
            tool_samples.len(),
            tool_selector_path.display()
        );
    }

    // Write outcome JSONL
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&outcome_path)?;
        for sample in &outcome_samples {
            writeln!(f, "{}", serde_json::to_string(sample)?)?;
        }
        tracing::info!(
            "wrote {} outcome samples to {}",
            outcome_samples.len(),
            outcome_path.display()
        );
    }

    tracing::info!("export complete → {}", output.display());
    Ok(())
}
