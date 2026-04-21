use std::path::Path;

use anyhow::{Context, Result};
use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::Dataset;
use burn::module::Module;
use burn::optim::AdamConfig;
use burn::record::CompactRecorder;
use burn::tensor::backend::AutodiffBackend;
use burn::train::LearnerBuilder;
use burn::train::metric::{AccuracyMetric, LossMetric};

use crate::export::dataset::{
    OutcomeBatcher, OutcomeDataset, ToolSelectorBatcher, ToolSelectorDataset,
};
use crate::models::common::SessionEncoderConfig;
use crate::models::outcome_predictor::OutcomePredictorConfig;
use crate::models::tool_selector::ToolSelectorConfig;

use super::TrainConfig;

/// Train the tool selector model.
pub fn train_tool_selector<B: AutodiffBackend>(config: TrainConfig) -> Result<()> {
    let device = B::Device::default();

    // Load dataset
    let data_path = Path::new(&config.data_dir).join("tool_selector.jsonl");
    let dataset = ToolSelectorDataset::from_jsonl(&data_path)
        .context("failed to load tool_selector.jsonl")?;

    tracing::info!("loaded {} tool selector samples", dataset.len());

    let (train_ds, val_ds) = dataset.split(0.15);
    tracing::info!(
        "split: {} train, {} validation",
        train_ds.len(),
        val_ds.len()
    );

    // Build data loaders
    let batcher_train = ToolSelectorBatcher::<B>::new(device.clone());
    let batcher_valid = ToolSelectorBatcher::<B::InnerBackend>::new(device.clone());

    let dataloader_train = DataLoaderBuilder::new(batcher_train)
        .batch_size(config.batch_size)
        .shuffle(42)
        .num_workers(2)
        .build(train_ds);

    let dataloader_valid = DataLoaderBuilder::new(batcher_valid)
        .batch_size(config.batch_size)
        .num_workers(1)
        .build(val_ds);

    // Build model
    let model_config = ToolSelectorConfig::new(SessionEncoderConfig::new());
    let model = model_config.init::<B>(&device);

    // Build learner
    let artifact_dir = &config.output_dir;
    std::fs::create_dir_all(artifact_dir)?;

    let learner = LearnerBuilder::new(artifact_dir)
        .metric_train_numeric(AccuracyMetric::new())
        .metric_valid_numeric(AccuracyMetric::new())
        .metric_train_numeric(LossMetric::new())
        .metric_valid_numeric(LossMetric::new())
        .with_file_checkpointer(CompactRecorder::new())
        .devices(vec![device])
        .num_epochs(config.epochs)
        .build(model, AdamConfig::new().init(), config.learning_rate);

    let trained_model = learner.fit(dataloader_train, dataloader_valid);

    // Save final model
    trained_model
        .save_file(
            Path::new(artifact_dir).join("tool_selector_final"),
            &CompactRecorder::new(),
        )
        .context("failed to save trained model")?;

    tracing::info!("training complete, model saved to {artifact_dir}");
    Ok(())
}

/// Train the outcome predictor model.
pub fn train_outcome_predictor<B: AutodiffBackend>(config: TrainConfig) -> Result<()> {
    let device = B::Device::default();

    let data_path = Path::new(&config.data_dir).join("outcome.jsonl");
    let dataset =
        OutcomeDataset::from_jsonl(&data_path).context("failed to load outcome.jsonl")?;

    tracing::info!("loaded {} outcome samples", dataset.len());

    let (train_ds, val_ds) = dataset.split(0.15);
    tracing::info!(
        "split: {} train, {} validation",
        train_ds.len(),
        val_ds.len()
    );

    let batcher_train = OutcomeBatcher::<B>::new(device.clone());
    let batcher_valid = OutcomeBatcher::<B::InnerBackend>::new(device.clone());

    let dataloader_train = DataLoaderBuilder::new(batcher_train)
        .batch_size(config.batch_size)
        .shuffle(42)
        .num_workers(2)
        .build(train_ds);

    let dataloader_valid = DataLoaderBuilder::new(batcher_valid)
        .batch_size(config.batch_size)
        .num_workers(1)
        .build(val_ds);

    let model_config = OutcomePredictorConfig::new(SessionEncoderConfig::new());
    let model = model_config.init::<B>(&device);

    let artifact_dir = &config.output_dir;
    std::fs::create_dir_all(artifact_dir)?;

    let learner = LearnerBuilder::new(artifact_dir)
        .metric_train_numeric(AccuracyMetric::new())
        .metric_valid_numeric(AccuracyMetric::new())
        .metric_train_numeric(LossMetric::new())
        .metric_valid_numeric(LossMetric::new())
        .with_file_checkpointer(CompactRecorder::new())
        .devices(vec![device])
        .num_epochs(config.epochs)
        .build(model, AdamConfig::new().init(), config.learning_rate);

    let trained_model = learner.fit(dataloader_train, dataloader_valid);

    trained_model
        .save_file(
            Path::new(artifact_dir).join("outcome_predictor_final"),
            &CompactRecorder::new(),
        )
        .context("failed to save trained model")?;

    tracing::info!("training complete, model saved to {artifact_dir}");
    Ok(())
}
