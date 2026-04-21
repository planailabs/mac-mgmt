use std::path::Path;

use anyhow::{Context, Result};
use burn::module::Module;
use burn::record::CompactRecorder;
use burn::tensor::backend::Backend;

use crate::export::dataset::{ToolSelectorBatcher, ToolSelectorDataset};
use crate::export::tokenizer::NUM_TOOLS;
use crate::models::common::SessionEncoderConfig;
use crate::models::tool_selector::{ToolSelector, ToolSelectorConfig};

/// Evaluate a trained tool selector model on held-out data.
pub fn eval_tool_selector<B: Backend>(checkpoint_path: &str, data_dir: &str) -> Result<()> {
    let device = B::Device::default();

    // Load model
    let model_config = ToolSelectorConfig::new(SessionEncoderConfig::new());
    let model: ToolSelector<B> = model_config
        .init(&device)
        .load_file(checkpoint_path, &CompactRecorder::new(), &device)
        .context("failed to load model checkpoint")?;

    // Load validation data
    let data_path = Path::new(data_dir).join("tool_selector.jsonl");
    let dataset = ToolSelectorDataset::from_jsonl(&data_path)
        .context("failed to load tool_selector.jsonl")?;

    let (_train_ds, val_ds) = dataset.split(0.15);
    tracing::info!("evaluating on {} validation samples", val_ds.len());

    // Simple evaluation: iterate and compute top-1 and top-3 accuracy
    let batcher = ToolSelectorBatcher::<B>::new(device);
    let mut correct_top1 = 0usize;
    let mut correct_top3 = 0usize;
    let mut total = 0usize;

    use burn::data::dataloader::batcher::Batcher;
    use burn::data::dataset::Dataset;

    // Process in batches of 32
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

            // Top-1
            let pred = row
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);
            if pred == target {
                correct_top1 += 1;
            }

            // Top-3
            let mut indexed: Vec<(usize, f32)> =
                row.into_iter().enumerate().collect();
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
        "evaluation results: top-1 accuracy = {:.2}%, top-3 accuracy = {:.2}% ({} samples)",
        top1_acc * 100.0,
        top3_acc * 100.0,
        total,
    );

    // Random baseline for reference
    let random_top1 = 1.0 / NUM_TOOLS as f64;
    let random_top3 = 3.0 / NUM_TOOLS as f64;
    tracing::info!(
        "random baseline: top-1 = {:.2}%, top-3 = {:.2}%",
        random_top1 * 100.0,
        random_top3 * 100.0,
    );

    Ok(())
}
