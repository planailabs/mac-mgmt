mod extractor;
pub mod tokenizer;
pub mod features;
pub mod dataset;
pub mod sft;
pub mod curriculum;
pub mod augment;

pub use extractor::ExportedSession;

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

/// Write a slice of serializable items as newline-delimited JSON.
fn write_jsonl_file<T: Serialize>(path: &Path, items: &[T]) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    for item in items {
        writeln!(f, "{}", serde_json::to_string(item)?)?;
    }
    Ok(())
}

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
    let mut issue_samples = Vec::new();
    let mut embedder_samples = Vec::new();

    for session in &sessions {
        // Full session export
        let session_json = serde_json::to_string(session)?;
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&sessions_path)?;
            writeln!(f, "{session_json}")?;
        }

        // Tool selector samples (one per tool call in the session)
        let ts = features::extract_tool_selector_samples(session, &vocab);
        tool_samples.extend(ts);

        // Outcome sample (one per session)
        if let Some(os) = features::extract_outcome_sample(session, &vocab) {
            outcome_samples.push(os);
        }

        // Issue classifier samples (one per staff_ping)
        let ics = features::extract_issue_classifier_samples(session, &vocab);
        issue_samples.extend(ics);

        // Embedder sample (one per session)
        if let Some(es) = features::extract_embedder_sample(session, &vocab) {
            embedder_samples.push(es);
        }
    }

    write_jsonl_file(&tool_selector_path, &tool_samples)?;
    tracing::info!(
        "wrote {} tool selector samples to {}",
        tool_samples.len(),
        tool_selector_path.display()
    );

    write_jsonl_file(&outcome_path, &outcome_samples)?;
    tracing::info!(
        "wrote {} outcome samples to {}",
        outcome_samples.len(),
        outcome_path.display()
    );

    let issue_path = output.join("issue_classifier.jsonl");
    write_jsonl_file(&issue_path, &issue_samples)?;
    tracing::info!(
        "wrote {} issue classifier samples to {}",
        issue_samples.len(),
        issue_path.display()
    );

    let embedder_path = output.join("embedder.jsonl");
    write_jsonl_file(&embedder_path, &embedder_samples)?;
    tracing::info!(
        "wrote {} embedder samples to {}",
        embedder_samples.len(),
        embedder_path.display()
    );

    tracing::info!("export complete → {}", output.display());
    Ok(())
}

/// Run the SFT export pipeline: sessions → ChatML conversations.
///
/// Can read from DB (db_url) or from a previously exported sessions.jsonl (input_dir).
pub async fn run_export_sft(
    db_url: Option<&str>,
    input_dir: Option<&str>,
    output_dir: &str,
    system_template: Option<&str>,
    embedder_checkpoint: Option<&str>,
    do_augment: bool,
    min_messages: usize,
) -> Result<()> {
    let output = Path::new(output_dir);
    std::fs::create_dir_all(output)?;

    // Load sessions from DB or from existing JSONL
    let sessions = if let Some(url) = db_url {
        let pool = sqlx::PgPool::connect(url)
            .await
            .context("failed to connect to database")?;
        extractor::extract_all(&pool, min_messages).await?
    } else if let Some(dir) = input_dir {
        let sessions_path = Path::new(dir).join("sessions.jsonl");
        let content = std::fs::read_to_string(&sessions_path)
            .context("failed to read sessions.jsonl")?;
        content
            .lines()
            .filter(|l| !l.is_empty())
            .map(serde_json::from_str)
            .collect::<std::result::Result<Vec<ExportedSession>, _>>()
            .context("failed to parse sessions.jsonl")?
    } else {
        anyhow::bail!("either --db-url or --input-dir is required");
    };

    tracing::info!("loaded {} sessions", sessions.len());
    if sessions.is_empty() {
        tracing::warn!("no sessions found");
        return Ok(());
    }

    // Generate curriculum (uses embedder if checkpoint provided)
    let data_dir = input_dir.unwrap_or(output_dir);
    let curriculum = curriculum::generate_curriculum(
        &sessions,
        embedder_checkpoint,
        data_dir,
    )?;

    // Save curriculum
    let curriculum_path = output.join("curriculum.json");
    std::fs::write(
        &curriculum_path,
        serde_json::to_string_pretty(&curriculum)?,
    )?;
    tracing::info!(
        "generated curriculum for {} sessions → {}",
        curriculum.entries.len(),
        curriculum_path.display()
    );

    // Convert to SFT format
    let mut conversations = sft::convert_sessions(&sessions, system_template);
    tracing::info!("converted {} conversations", conversations.len());

    // Apply curriculum metadata
    let curriculum_map: std::collections::HashMap<String, &curriculum::CurriculumEntry> =
        curriculum
            .entries
            .iter()
            .map(|e| (e.session_id.clone(), e))
            .collect();

    for conv in &mut conversations {
        if let Some(entry) = curriculum_map.get(&conv.metadata.session_id) {
            conv.metadata.curriculum_rank = Some(entry.curriculum_rank);
            conv.metadata.sample_weight = Some(entry.sample_weight);
        }
    }

    // Sort by curriculum rank
    conversations.sort_by_key(|c| c.metadata.curriculum_rank.unwrap_or(usize::MAX));

    // Augment if requested
    let conversations = if do_augment {
        let config = augment::AugmentConfig::default();
        let augmented = augment::augment(&conversations, &config);
        tracing::info!(
            "augmented {} → {} conversations",
            conversations.len(),
            augmented.len()
        );
        augmented
    } else {
        conversations
    };

    // Write output
    let sft_path = output.join("sft_conversations.jsonl");
    write_jsonl_file(&sft_path, &conversations)?;
    tracing::info!(
        "wrote {} SFT conversations to {}",
        conversations.len(),
        sft_path.display()
    );

    Ok(())
}
