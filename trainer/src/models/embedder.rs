use burn::config::Config;
use burn::module::Module;
use burn::nn::{Linear, LinearConfig};
use burn::tensor::Tensor;
use burn::tensor::backend::{AutodiffBackend, Backend};
use burn::train::{TrainOutput, TrainStep, ValidStep};

use crate::export::dataset::EmbedderBatch;

use super::common::{SessionEncoder, SessionEncoderConfig};

/// Embedding dimension for the output vectors.
pub const EMBED_DIM: usize = 256;

/// Configuration for the session embedder model.
#[derive(Config, Debug)]
pub struct SessionEmbedderConfig {
    pub encoder: SessionEncoderConfig,
    /// Output embedding dimension.
    #[config(default = 256)]
    pub embed_dim: usize,
}

/// Produces fixed-size embedding vectors for sessions.
///
/// Trained with contrastive loss: sessions with the same outcome label
/// should have similar embeddings, different labels should be far apart.
#[derive(Module, Debug)]
pub struct SessionEmbedder<B: Backend> {
    encoder: SessionEncoder<B>,
    projection: Linear<B>,
}

impl SessionEmbedderConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> SessionEmbedder<B> {
        let d_model = self.encoder.d_model;
        let encoder = self.encoder.init(device);
        let projection = LinearConfig::new(d_model, self.embed_dim).init(device);

        SessionEmbedder {
            encoder,
            projection,
        }
    }
}

impl<B: Backend> SessionEmbedder<B> {
    /// Produce L2-normalized embeddings [batch, embed_dim].
    pub fn forward(&self, batch: &EmbedderBatch<B>) -> Tensor<B, 2> {
        let encoded = self.encoder.forward(
            batch.tokens.clone(),
            batch.roles.clone(),
            batch.mask.clone(),
        );
        let pooled = self.encoder.pool(encoded, batch.mask.clone());
        let projected = self.projection.forward(pooled);
        // L2 normalize
        let norm = projected
            .clone()
            .powf_scalar(2.0)
            .sum_dim(1)
            .clamp_min(1e-12)
            .sqrt();
        projected / norm
    }

    /// Contrastive loss: same-label pairs are pulled together, different-label pairs pushed apart.
    ///
    /// Uses a simplified NT-Xent (normalized temperature-scaled cross entropy) loss.
    /// Within a batch, we treat samples with the same label as positives.
    pub fn forward_contrastive(&self, batch: &EmbedderBatch<B>) -> ContrastiveOutput<B> {
        let embeddings = self.forward(batch);
        let labels = batch.labels.clone();
        let temperature = 0.07;

        let [batch_size, _embed_dim] = embeddings.dims();

        // Cosine similarity matrix: [batch, batch]
        let sim = embeddings
            .clone()
            .matmul(embeddings.clone().swap_dims(0, 1))
            / temperature;

        // Build positive mask: 1 where labels[i] == labels[j] and i != j
        // We do this by comparing label tensors
        let labels_data = labels.to_data();
        let labels_vec: Vec<i64> = labels_data.as_slice().unwrap().to_vec();

        let mut pos_mask_data = vec![0.0f32; batch_size * batch_size];
        for i in 0..batch_size {
            for j in 0..batch_size {
                if i != j && labels_vec[i] == labels_vec[j] {
                    pos_mask_data[i * batch_size + j] = 1.0;
                }
            }
        }

        let device = sim.device();
        let pos_mask = Tensor::<B, 2>::from_data(
            burn::tensor::TensorData::new(pos_mask_data, [batch_size, batch_size]),
            &device,
        );

        // Self-mask: large negative for diagonal
        let mut self_mask_data = vec![0.0f32; batch_size * batch_size];
        for i in 0..batch_size {
            self_mask_data[i * batch_size + i] = -1e9;
        }
        let self_mask = Tensor::<B, 2>::from_data(
            burn::tensor::TensorData::new(self_mask_data, [batch_size, batch_size]),
            &device,
        );

        let sim = sim + self_mask;

        // Log-sum-exp denominator
        let log_sum_exp = sim.clone().exp().sum_dim(1).clamp_min(1e-12).log(); // [batch, 1]

        // For each anchor, average the similarity to its positives
        let pos_count = pos_mask.clone().sum_dim(1).clamp_min(1.0); // [batch, 1]
        let pos_sim_sum = (sim * pos_mask).sum_dim(1); // [batch, 1]
        let pos_avg = pos_sim_sum / pos_count;

        // Loss = -mean(pos_avg - log_sum_exp)
        let per_sample_loss = log_sum_exp - pos_avg;
        let loss = per_sample_loss.mean();

        ContrastiveOutput {
            loss,
            embeddings,
            labels,
        }
    }
}

/// Output of contrastive training step.
#[derive(Debug)]
pub struct ContrastiveOutput<B: Backend> {
    pub loss: Tensor<B, 1>,
    pub embeddings: Tensor<B, 2>,
    pub labels: Tensor<B, 1, burn::tensor::Int>,
}

/// Regression output wrapper for Burn's Learner (uses loss metric).
impl<B: AutodiffBackend> TrainStep<EmbedderBatch<B>, burn::train::RegressionOutput<B>>
    for SessionEmbedder<B>
{
    fn step(&self, batch: EmbedderBatch<B>) -> TrainOutput<burn::train::RegressionOutput<B>> {
        let out = self.forward_contrastive(&batch);
        // Pack contrastive loss into RegressionOutput (no meaningful output/targets)
        let [batch_size, embed_dim] = out.embeddings.dims();
        let regression = burn::train::RegressionOutput {
            loss: out.loss.clone(),
            output: out.embeddings,
            targets: Tensor::zeros([batch_size, embed_dim], &batch.tokens.device()),
        };
        TrainOutput::new(self, out.loss.backward(), regression)
    }
}

impl<B: Backend> ValidStep<EmbedderBatch<B>, burn::train::RegressionOutput<B>>
    for SessionEmbedder<B>
{
    fn step(&self, batch: EmbedderBatch<B>) -> burn::train::RegressionOutput<B> {
        let out = self.forward_contrastive(&batch);
        let [batch_size, embed_dim] = out.embeddings.dims();
        burn::train::RegressionOutput {
            loss: out.loss,
            output: out.embeddings,
            targets: Tensor::zeros([batch_size, embed_dim], &batch.tokens.device()),
        }
    }
}
