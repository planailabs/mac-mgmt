pub mod integration;

use std::path::Path;

use anyhow::{Context, Result};
use burn::data::dataloader::batcher::Batcher;
use burn::data::dataset::Dataset;
use burn::module::Module;
use burn::record::CompactRecorder;
use burn::tensor::backend::Backend;

use crate::export::dataset::{
    EmbedderBatcher, EmbedderDataset, IssueClassifierBatcher, IssueClassifierDataset,
    OutcomeBatcher, OutcomeDataset, ToolSelectorBatcher, ToolSelectorDataset,
};
use crate::export::tokenizer::NUM_TOOLS;
use crate::models::common::SessionEncoderConfig;
use crate::models::embedder::{SessionEmbedder, SessionEmbedderConfig};
use crate::models::issue_classifier::{IssueClassifier, IssueClassifierConfig, NUM_CATEGORIES};
use crate::models::outcome_predictor::{NUM_OUTCOMES, OutcomePredictor, OutcomePredictorConfig};
use crate::models::tool_selector::{ToolSelector, ToolSelectorConfig};

/// Evaluate a trained tool selector model on held-out data.
pub fn eval_tool_selector<B: Backend>(checkpoint_path: &str, data_dir: &str) -> Result<()> {
    let device = B::Device::default();

    let model_config = ToolSelectorConfig::new(SessionEncoderConfig::new());
    let model: ToolSelector<B> = model_config
        .init(&device)
        .load_file(checkpoint_path, &CompactRecorder::new(), &device)
        .context("failed to load model checkpoint")?;

    let data_path = Path::new(data_dir).join("tool_selector.jsonl");
    let dataset = ToolSelectorDataset::from_jsonl(&data_path)
        .context("failed to load tool_selector.jsonl")?;

    let (_train_ds, val_ds) = dataset.split(0.15);
    tracing::info!("evaluating on {} validation samples", val_ds.len());

    let batcher = ToolSelectorBatcher::<B>::new(device);
    let mut correct_top1 = 0usize;
    let mut correct_top3 = 0usize;
    let mut total = 0usize;

    let batch_size = 32;
    let n = val_ds.len();
    let mut idx = 0;
    while idx < n {
        let end = (idx + batch_size).min(n);
        let items: Vec<_> = (idx..end).filter_map(|i| val_ds.get(i)).collect();
        let targets: Vec<usize> = items.iter().map(|s| s.target_tool).collect();
        let batch = batcher.batch(items);

        let logits = model.forward(&batch);
        let logits_data = logits.to_data();

        for (i, &target) in targets.iter().enumerate() {
            let row_start = i * NUM_TOOLS;
            let row: Vec<f32> = (0..NUM_TOOLS)
                .map(|j| logits_data.as_slice::<f32>().unwrap()[row_start + j])
                .collect();

            let pred = row
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);
            if pred == target {
                correct_top1 += 1;
            }

            let mut indexed: Vec<(usize, f32)> = row.into_iter().enumerate().collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            if indexed.iter().take(3).any(|(i, _)| *i == target) {
                correct_top3 += 1;
            }

            total += 1;
        }

        idx = end;
    }

    let top1_acc = if total > 0 {
        correct_top1 as f64 / total as f64
    } else {
        0.0
    };
    let top3_acc = if total > 0 {
        correct_top3 as f64 / total as f64
    } else {
        0.0
    };

    tracing::info!(
        "tool selector: top-1 = {:.2}%, top-3 = {:.2}% ({} samples)",
        top1_acc * 100.0,
        top3_acc * 100.0,
        total,
    );

    let random_top1 = 1.0 / NUM_TOOLS as f64;
    let random_top3 = 3.0 / NUM_TOOLS as f64;
    tracing::info!(
        "random baseline: top-1 = {:.2}%, top-3 = {:.2}%",
        random_top1 * 100.0,
        random_top3 * 100.0,
    );

    Ok(())
}

/// Evaluate a trained outcome predictor on held-out data.
pub fn eval_outcome_predictor<B: Backend>(checkpoint_path: &str, data_dir: &str) -> Result<()> {
    let device = B::Device::default();

    let model_config = OutcomePredictorConfig::new(SessionEncoderConfig::new());
    let model: OutcomePredictor<B> = model_config
        .init(&device)
        .load_file(checkpoint_path, &CompactRecorder::new(), &device)
        .context("failed to load model checkpoint")?;

    let data_path = Path::new(data_dir).join("outcome.jsonl");
    let dataset = OutcomeDataset::from_jsonl(&data_path).context("failed to load outcome.jsonl")?;

    let (_train_ds, val_ds) = dataset.split(0.15);
    tracing::info!("evaluating on {} validation samples", val_ds.len());

    let batcher = OutcomeBatcher::<B>::new(device);
    let mut correct = 0usize;
    let mut total = 0usize;
    let mut per_class_correct = [0usize; NUM_OUTCOMES];
    let mut per_class_total = [0usize; NUM_OUTCOMES];

    let batch_size = 32;
    let n = val_ds.len();
    let mut idx = 0;
    while idx < n {
        let end = (idx + batch_size).min(n);
        let items: Vec<_> = (idx..end).filter_map(|i| val_ds.get(i)).collect();
        let targets: Vec<usize> = items.iter().map(|s| s.target).collect();
        let batch = batcher.batch(items);

        let logits = model.forward(&batch);
        let logits_data = logits.to_data();
        let logits_slice: &[f32] = logits_data.as_slice().unwrap();

        for (i, &target) in targets.iter().enumerate() {
            let row = &logits_slice[i * NUM_OUTCOMES..(i + 1) * NUM_OUTCOMES];
            let pred = row
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);

            if pred == target {
                correct += 1;
                per_class_correct[target] += 1;
            }
            per_class_total[target] += 1;
            total += 1;
        }

        idx = end;
    }

    let accuracy = if total > 0 {
        correct as f64 / total as f64
    } else {
        0.0
    };

    tracing::info!(
        "outcome predictor: accuracy = {:.2}% ({} samples)",
        accuracy * 100.0,
        total,
    );

    let class_names = ["done/completed", "failed", "needs_human/cancelled/paused"];
    for (i, name) in class_names.iter().enumerate() {
        let acc = if per_class_total[i] > 0 {
            per_class_correct[i] as f64 / per_class_total[i] as f64
        } else {
            0.0
        };
        tracing::info!(
            "  {}: {:.2}% ({}/{})",
            name,
            acc * 100.0,
            per_class_correct[i],
            per_class_total[i],
        );
    }

    Ok(())
}

/// Evaluate a trained issue classifier on held-out data.
pub fn eval_issue_classifier<B: Backend>(checkpoint_path: &str, data_dir: &str) -> Result<()> {
    let device = B::Device::default();

    let model_config = IssueClassifierConfig::new(SessionEncoderConfig::new());
    let model: IssueClassifier<B> = model_config
        .init(&device)
        .load_file(checkpoint_path, &CompactRecorder::new(), &device)
        .context("failed to load model checkpoint")?;

    let data_path = Path::new(data_dir).join("issue_classifier.jsonl");
    let dataset = IssueClassifierDataset::from_jsonl(&data_path)
        .context("failed to load issue_classifier.jsonl")?;

    let (_train_ds, val_ds) = dataset.split(0.15);
    tracing::info!("evaluating on {} validation samples", val_ds.len());

    let batcher = IssueClassifierBatcher::<B>::new(device);
    let mut correct = 0usize;
    let mut total = 0usize;

    let batch_size = 32;
    let n = val_ds.len();
    let mut idx = 0;
    while idx < n {
        let end = (idx + batch_size).min(n);
        let items: Vec<_> = (idx..end).filter_map(|i| val_ds.get(i)).collect();
        let targets: Vec<usize> = items.iter().map(|s| s.target).collect();
        let batch = batcher.batch(items);

        let logits = model.forward(&batch);
        let logits_data = logits.to_data();
        let logits_slice: &[f32] = logits_data.as_slice().unwrap();

        for (i, &target) in targets.iter().enumerate() {
            let row = &logits_slice[i * NUM_CATEGORIES..(i + 1) * NUM_CATEGORIES];
            let pred = row
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);

            if pred == target {
                correct += 1;
            }
            total += 1;
        }

        idx = end;
    }

    let accuracy = if total > 0 {
        correct as f64 / total as f64
    } else {
        0.0
    };

    tracing::info!(
        "issue classifier: accuracy = {:.2}% ({} samples)",
        accuracy * 100.0,
        total,
    );

    let random = 1.0 / NUM_CATEGORIES as f64;
    tracing::info!("random baseline: {:.2}%", random * 100.0);

    Ok(())
}

/// Evaluate a trained session embedder by checking embedding cluster quality.
pub fn eval_embedder<B: Backend>(checkpoint_path: &str, data_dir: &str) -> Result<()> {
    let device = B::Device::default();

    let model_config = SessionEmbedderConfig::new(SessionEncoderConfig::new());
    let model: SessionEmbedder<B> = model_config
        .init(&device)
        .load_file(checkpoint_path, &CompactRecorder::new(), &device)
        .context("failed to load model checkpoint")?;

    let data_path = Path::new(data_dir).join("embedder.jsonl");
    let dataset =
        EmbedderDataset::from_jsonl(&data_path).context("failed to load embedder.jsonl")?;

    let (_train_ds, val_ds) = dataset.split(0.15);
    tracing::info!("evaluating on {} validation samples", val_ds.len());

    let batcher = EmbedderBatcher::<B>::new(device);

    // Collect all embeddings and labels
    let mut all_embeddings: Vec<Vec<f32>> = Vec::new();
    let mut all_labels: Vec<usize> = Vec::new();

    let batch_size = 32;
    let n = val_ds.len();
    let mut idx = 0;
    while idx < n {
        let end = (idx + batch_size).min(n);
        let items: Vec<_> = (idx..end).filter_map(|i| val_ds.get(i)).collect();
        let labels: Vec<usize> = items.iter().map(|s| s.label).collect();
        let batch = batcher.batch(items);

        let embeddings = model.forward(&batch);
        let embed_data = embeddings.to_data();
        let embed_slice: &[f32] = embed_data.as_slice().unwrap();
        let [cur_batch, embed_dim] = embeddings.dims();

        for i in 0..cur_batch {
            let emb: Vec<f32> = embed_slice[i * embed_dim..(i + 1) * embed_dim].to_vec();
            all_embeddings.push(emb);
        }
        all_labels.extend(labels);

        idx = end;
    }

    // Compute average intra-class and inter-class cosine similarity
    let n_samples = all_embeddings.len();
    if n_samples < 2 {
        tracing::warn!("not enough samples for embedding evaluation");
        return Ok(());
    }

    let mut intra_sim_sum = 0.0f64;
    let mut intra_count = 0usize;
    let mut inter_sim_sum = 0.0f64;
    let mut inter_count = 0usize;

    for i in 0..n_samples {
        for j in (i + 1)..n_samples {
            let sim = cosine_similarity(&all_embeddings[i], &all_embeddings[j]);
            if all_labels[i] == all_labels[j] {
                intra_sim_sum += sim;
                intra_count += 1;
            } else {
                inter_sim_sum += sim;
                inter_count += 1;
            }
        }
    }

    let intra_avg = if intra_count > 0 {
        intra_sim_sum / intra_count as f64
    } else {
        0.0
    };
    let inter_avg = if inter_count > 0 {
        inter_sim_sum / inter_count as f64
    } else {
        0.0
    };

    tracing::info!(
        "embedder: avg intra-class similarity = {:.4}, avg inter-class = {:.4}, gap = {:.4} ({} samples)",
        intra_avg,
        inter_avg,
        intra_avg - inter_avg,
        n_samples,
    );

    Ok(())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| *x as f64 * *y as f64)
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm_a < 1e-12 || norm_b < 1e-12 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
