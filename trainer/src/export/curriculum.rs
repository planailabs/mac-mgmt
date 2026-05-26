//! Autoencoder-driven curriculum design using the SessionEmbedder.
//!
//! Embeds all sessions, clusters them, and produces a difficulty-ordered
//! training schedule with per-session quality weights.

use std::path::Path;

use anyhow::{Context, Result};
use burn::data::dataloader::batcher::Batcher;
use burn::module::Module;
use burn::record::CompactRecorder;
use burn::tensor::backend::Backend;
use serde::{Deserialize, Serialize};

use super::ExportedSession;
use super::dataset::EmbedderBatcher;
use super::features;
use super::tokenizer::Vocabulary;
use crate::models::common::SessionEncoderConfig;
use crate::models::embedder::{SessionEmbedder, SessionEmbedderConfig};

/// Curriculum metadata for a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurriculumEntry {
    pub session_id: String,
    pub outcome: String,
    /// Rank in curriculum order (0 = easiest / highest quality).
    pub curriculum_rank: usize,
    /// Training weight (0.0–1.0). Outliers get downweighted.
    pub sample_weight: f64,
    /// Raw difficulty score (higher = harder).
    pub difficulty_score: f64,
    /// Distance to own cluster centroid.
    pub cluster_distance: f64,
    /// Assigned cluster (0, 1, 2).
    pub cluster: usize,
}

/// Full curriculum output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curriculum {
    pub entries: Vec<CurriculumEntry>,
}

/// Generate curriculum from sessions using a trained embedder.
///
/// If no embedder checkpoint is available, falls back to a heuristic
/// based on session length and outcome.
pub fn generate_curriculum(
    sessions: &[ExportedSession],
    embedder_checkpoint: Option<&str>,
    data_dir: &str,
) -> Result<Curriculum> {
    if let Some(checkpoint) = embedder_checkpoint {
        generate_with_embedder::<burn::backend::NdArray>(sessions, checkpoint, data_dir)
    } else {
        Ok(generate_heuristic(sessions))
    }
}

/// Embedder-based curriculum: embed → cluster → score → order.
fn generate_with_embedder<B: Backend>(
    sessions: &[ExportedSession],
    checkpoint_path: &str,
    data_dir: &str,
) -> Result<Curriculum> {
    let device = B::Device::default();

    // Load embedder
    let config = SessionEmbedderConfig::new(SessionEncoderConfig::new());
    let model: SessionEmbedder<B> = config
        .init(&device)
        .load_file(checkpoint_path, &CompactRecorder::new(), &device)
        .context("failed to load embedder checkpoint")?;

    // Load vocab
    let vocab_path = Path::new(data_dir).join("vocab.json");
    let vocab_json = std::fs::read_to_string(&vocab_path).context("failed to read vocab.json")?;
    let vocab: Vocabulary = serde_json::from_str(&vocab_json).context("failed to parse vocab")?;

    // Build embedder samples and extract embeddings
    let mut embeddings: Vec<Vec<f32>> = Vec::new();
    let mut labels: Vec<usize> = Vec::new();

    let batcher = EmbedderBatcher::<B>::new(device);

    // Process in batches
    let samples: Vec<_> = sessions
        .iter()
        .filter_map(|s| features::extract_embedder_sample(s, &vocab))
        .collect();

    let batch_size = 32;
    let mut idx = 0;
    while idx < samples.len() {
        let end = (idx + batch_size).min(samples.len());
        let batch_items: Vec<_> = samples[idx..end].to_vec();
        let batch_labels: Vec<_> = batch_items.iter().map(|s| s.label).collect();
        let batch = batcher.batch(batch_items);

        let emb = model.forward(&batch);
        let emb_data = emb.to_data();
        let emb_slice: &[f32] = emb_data.as_slice().unwrap();
        let [cur_batch, embed_dim] = emb.dims();

        for i in 0..cur_batch {
            embeddings.push(emb_slice[i * embed_dim..(i + 1) * embed_dim].to_vec());
        }
        labels.extend(batch_labels);

        idx = end;
    }

    // If we got fewer embeddings than sessions (some filtered out), fall back
    if embeddings.len() < sessions.len() / 2 {
        tracing::warn!(
            "only embedded {}/{} sessions, falling back to heuristic",
            embeddings.len(),
            sessions.len()
        );
        return Ok(generate_heuristic(sessions));
    }

    // K-means clustering (k=3)
    let k = 3;
    let centroids = kmeans(&embeddings, k, 20);

    // Assign clusters and compute distances
    let mut cluster_assignments = Vec::new();
    let mut distances = Vec::new();
    for emb in &embeddings {
        let (cluster, dist) = nearest_centroid(emb, &centroids);
        cluster_assignments.push(cluster);
        distances.push(dist);
    }

    // Compute per-cluster stats for outlier detection
    let mut cluster_mean_dist = vec![0.0f64; k];
    let mut cluster_std_dist = vec![0.0f64; k];
    let mut cluster_counts = vec![0usize; k];

    for (i, &c) in cluster_assignments.iter().enumerate() {
        cluster_mean_dist[c] += distances[i];
        cluster_counts[c] += 1;
    }
    for c in 0..k {
        if cluster_counts[c] > 0 {
            cluster_mean_dist[c] /= cluster_counts[c] as f64;
        }
    }
    for (i, &c) in cluster_assignments.iter().enumerate() {
        cluster_std_dist[c] += (distances[i] - cluster_mean_dist[c]).powi(2);
    }
    for c in 0..k {
        if cluster_counts[c] > 1 {
            cluster_std_dist[c] = (cluster_std_dist[c] / (cluster_counts[c] - 1) as f64).sqrt();
        }
    }

    // Build entries — match back to sessions by index
    // We only have embeddings for sessions that had valid embedder samples
    let mut embedded_session_indices: Vec<usize> = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        if features::extract_embedder_sample(s, &vocab).is_some() {
            embedded_session_indices.push(i);
        }
    }

    let mut entries: Vec<CurriculumEntry> = Vec::new();
    for (emb_idx, &sess_idx) in embedded_session_indices.iter().enumerate() {
        if emb_idx >= embeddings.len() {
            break;
        }
        let session = &sessions[sess_idx];
        let cluster = cluster_assignments[emb_idx];
        let dist = distances[emb_idx];

        // Difficulty: combine cluster distance, session length, failed tool ratio
        let msg_count = session.messages.len() as f64;
        let failed_tools = session
            .messages
            .iter()
            .filter(|m| {
                m.metadata
                    .as_ref()
                    .and_then(|md| md.get("status"))
                    .and_then(|s| s.as_str())
                    == Some("error")
            })
            .count() as f64;
        let fail_ratio = if msg_count > 0.0 {
            failed_tools / msg_count
        } else {
            0.0
        };

        let difficulty = dist * 0.4 + (msg_count / 100.0) * 0.3 + fail_ratio * 0.3;

        // Weight: downweight outliers (>2 std from cluster mean)
        let z_score = if cluster_std_dist[cluster] > 1e-6 {
            (dist - cluster_mean_dist[cluster]) / cluster_std_dist[cluster]
        } else {
            0.0
        };
        let weight = if z_score > 2.0 {
            0.5
        } else if z_score > 1.5 {
            0.75
        } else {
            1.0
        };

        entries.push(CurriculumEntry {
            session_id: session.id.to_string(),
            outcome: session.state.clone(),
            curriculum_rank: 0, // filled after sorting
            sample_weight: weight,
            difficulty_score: difficulty,
            cluster_distance: dist,
            cluster,
        });
    }

    // Also add sessions that weren't embedded (with default weight)
    for (i, session) in sessions.iter().enumerate() {
        if !embedded_session_indices.contains(&i) {
            entries.push(CurriculumEntry {
                session_id: session.id.to_string(),
                outcome: session.state.clone(),
                curriculum_rank: 0,
                sample_weight: 0.8, // slightly downweighted — couldn't embed
                difficulty_score: session.messages.len() as f64 / 50.0,
                cluster_distance: 0.0,
                cluster: 0,
            });
        }
    }

    // Sort by difficulty (easy first) and assign ranks
    entries.sort_by(|a, b| a.difficulty_score.partial_cmp(&b.difficulty_score).unwrap());
    for (i, entry) in entries.iter_mut().enumerate() {
        entry.curriculum_rank = i;
    }

    Ok(Curriculum { entries })
}

/// Heuristic curriculum when no embedder is available.
fn generate_heuristic(sessions: &[ExportedSession]) -> Curriculum {
    let mut entries: Vec<CurriculumEntry> = sessions
        .iter()
        .map(|s| {
            let outcome_penalty = match s.state.as_str() {
                "done" | "completed" => 0.0,
                "failed" => 0.5,
                _ => 0.3,
            };
            let difficulty = s.messages.len() as f64 / 100.0 + outcome_penalty;

            CurriculumEntry {
                session_id: s.id.to_string(),
                outcome: s.state.clone(),
                curriculum_rank: 0,
                sample_weight: 1.0,
                difficulty_score: difficulty,
                cluster_distance: 0.0,
                cluster: 0,
            }
        })
        .collect();

    entries.sort_by(|a, b| a.difficulty_score.partial_cmp(&b.difficulty_score).unwrap());
    for (i, entry) in entries.iter_mut().enumerate() {
        entry.curriculum_rank = i;
    }

    Curriculum { entries }
}

// ── Simple k-means ─────────────────────────────────────────────────

fn kmeans(data: &[Vec<f32>], k: usize, max_iters: usize) -> Vec<Vec<f32>> {
    if data.is_empty() {
        return Vec::new();
    }
    let dim = data[0].len();

    // Initialize centroids by picking evenly spaced data points
    let mut centroids: Vec<Vec<f32>> = (0..k)
        .map(|i| {
            let idx = i * data.len() / k;
            data[idx].clone()
        })
        .collect();

    for _ in 0..max_iters {
        // Assign
        let mut assignments = vec![0usize; data.len()];
        for (i, point) in data.iter().enumerate() {
            assignments[i] = nearest_centroid(point, &centroids).0;
        }

        // Update centroids
        let mut new_centroids = vec![vec![0.0f32; dim]; k];
        let mut counts = vec![0usize; k];
        for (i, point) in data.iter().enumerate() {
            let c = assignments[i];
            counts[c] += 1;
            for (j, &val) in point.iter().enumerate() {
                new_centroids[c][j] += val;
            }
        }
        for c in 0..k {
            if counts[c] > 0 {
                for val in new_centroids[c].iter_mut().take(dim) {
                    *val /= counts[c] as f32;
                }
            }
        }

        // Check convergence
        let mut converged = true;
        for c in 0..k {
            let dist: f32 = centroids[c]
                .iter()
                .zip(&new_centroids[c])
                .map(|(a, b)| (a - b).powi(2))
                .sum();
            if dist > 1e-6 {
                converged = false;
            }
        }
        centroids = new_centroids;
        if converged {
            break;
        }
    }

    centroids
}

fn nearest_centroid(point: &[f32], centroids: &[Vec<f32>]) -> (usize, f64) {
    let mut best = 0;
    let mut best_dist = f64::MAX;
    for (i, centroid) in centroids.iter().enumerate() {
        let dist: f64 = point
            .iter()
            .zip(centroid)
            .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
            .sum();
        let dist = dist.sqrt();
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    (best, best_dist)
}
