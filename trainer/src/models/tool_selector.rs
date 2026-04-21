use burn::config::Config;
use burn::module::Module;
use burn::nn::{Dropout, DropoutConfig, Embedding, EmbeddingConfig, Linear, LinearConfig};
use burn::tensor::backend::{AutodiffBackend, Backend};
use burn::tensor::Tensor;
use burn::train::{ClassificationOutput, TrainOutput, TrainStep, ValidStep};

use crate::export::dataset::ToolSelectorBatch;
use crate::export::features::TOOL_HISTORY_LEN;
use crate::export::tokenizer::NUM_TOOLS;

use super::common::{SessionEncoder, SessionEncoderConfig};

/// Configuration for the tool selector model.
#[derive(Config, Debug)]
pub struct ToolSelectorConfig {
    /// Session encoder configuration.
    pub encoder: SessionEncoderConfig,
    /// Number of distinct phases.
    #[config(default = 12)]
    pub n_phases: usize,
}

/// Predicts the next tool to call given session context.
#[derive(Module, Debug)]
pub struct ToolSelector<B: Backend> {
    encoder: SessionEncoder<B>,
    phase_embedding: Embedding<B>,
    tool_history_embedding: Embedding<B>,
    history_projection: Linear<B>,
    classifier: Linear<B>,
    dropout: Dropout,
    #[module(skip)]
    d_model: usize,
}

impl ToolSelectorConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> ToolSelector<B> {
        let d_model = self.encoder.d_model;
        let encoder = self.encoder.init(device);
        // +1 for PAD tool index (NUM_TOOLS is used as padding)
        let tool_history_embedding =
            EmbeddingConfig::new(NUM_TOOLS + 1, d_model).init(device);
        let phase_embedding = EmbeddingConfig::new(self.n_phases, d_model).init(device);
        let history_projection =
            LinearConfig::new(d_model * TOOL_HISTORY_LEN, d_model).init(device);
        let classifier = LinearConfig::new(d_model * 3, NUM_TOOLS).init(device);
        let dropout = DropoutConfig::new(self.encoder.dropout).init();

        ToolSelector {
            encoder,
            phase_embedding,
            tool_history_embedding,
            history_projection,
            classifier,
            dropout,
            d_model,
        }
    }
}

impl<B: Backend> ToolSelector<B> {
    pub fn forward(&self, batch: &ToolSelectorBatch<B>) -> Tensor<B, 2> {
        let d_model = self.d_model;

        // 1. Encode context sequence → pooled representation
        let encoded = self.encoder.forward(
            batch.context.clone(),
            batch.roles.clone(),
            batch.mask.clone(),
        );
        let context_vec = self.encoder.pool(encoded, batch.mask.clone()); // [batch, d_model]

        // 2. Encode tool history
        let tool_emb = self.tool_history_embedding.forward(batch.tool_history.clone());
        // [batch, TOOL_HISTORY_LEN, d_model] → [batch, TOOL_HISTORY_LEN * d_model]
        let [batch_size, _, _] = tool_emb.dims();
        let tool_flat = tool_emb.reshape([batch_size, TOOL_HISTORY_LEN * d_model]);
        let history_vec = self.history_projection.forward(tool_flat); // [batch, d_model]

        // 3. Encode phase — phase is [batch] (1D int), embedding gives [batch, d_model] (2D)
        let phase_2d = batch.phase.clone().unsqueeze::<2>(); // [batch, 1]
        let phase_emb = self.phase_embedding.forward(phase_2d); // [batch, 1, d_model]
        let phase_vec: Tensor<B, 2> = phase_emb.squeeze(1); // [batch, d_model]

        // 4. Concatenate and classify
        let combined = Tensor::cat(vec![context_vec, history_vec, phase_vec], 1);
        let combined = self.dropout.forward(combined);
        self.classifier.forward(combined) // [batch, NUM_TOOLS]
    }

    pub fn forward_classification(
        &self,
        batch: &ToolSelectorBatch<B>,
    ) -> ClassificationOutput<B> {
        let logits = self.forward(batch);
        let targets = batch.targets.clone();
        let loss = burn::nn::loss::CrossEntropyLossConfig::new()
            .init(&logits.device())
            .forward(logits.clone(), targets.clone());

        ClassificationOutput {
            loss,
            output: logits,
            targets,
        }
    }
}

impl<B: AutodiffBackend> TrainStep<ToolSelectorBatch<B>, ClassificationOutput<B>>
    for ToolSelector<B>
{
    fn step(&self, batch: ToolSelectorBatch<B>) -> TrainOutput<ClassificationOutput<B>> {
        let output = self.forward_classification(&batch);
        TrainOutput::new(self, output.loss.backward(), output)
    }
}

impl<B: Backend> ValidStep<ToolSelectorBatch<B>, ClassificationOutput<B>>
    for ToolSelector<B>
{
    fn step(&self, batch: ToolSelectorBatch<B>) -> ClassificationOutput<B> {
        self.forward_classification(&batch)
    }
}
