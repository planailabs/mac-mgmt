use burn::config::Config;
use burn::module::Module;
use burn::nn::{Dropout, DropoutConfig, Linear, LinearConfig};
use burn::tensor::Tensor;
use burn::tensor::backend::{AutodiffBackend, Backend};
use burn::train::{ClassificationOutput, TrainOutput, TrainStep, ValidStep};

use crate::export::dataset::OutcomeBatch;

use super::common::{SessionEncoder, SessionEncoderConfig};

/// Number of outcome classes: done/completed=0, failed=1, needs_human/cancelled/paused=2.
pub const NUM_OUTCOMES: usize = 3;

/// Configuration for the outcome predictor model.
#[derive(Config, Debug)]
pub struct OutcomePredictorConfig {
    pub encoder: SessionEncoderConfig,
}

/// Predicts session outcome (success/failure/escalation) from early signals.
#[derive(Module, Debug)]
pub struct OutcomePredictor<B: Backend> {
    encoder: SessionEncoder<B>,
    classifier: Linear<B>,
    dropout: Dropout,
}

impl OutcomePredictorConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> OutcomePredictor<B> {
        let d_model = self.encoder.d_model;
        let encoder = self.encoder.init(device);
        let classifier = LinearConfig::new(d_model, NUM_OUTCOMES).init(device);
        let dropout = DropoutConfig::new(self.encoder.dropout).init();

        OutcomePredictor {
            encoder,
            classifier,
            dropout,
        }
    }
}

impl<B: Backend> OutcomePredictor<B> {
    pub fn forward(&self, batch: &OutcomeBatch<B>) -> Tensor<B, 2> {
        let encoded = self.encoder.forward(
            batch.tokens.clone(),
            batch.roles.clone(),
            batch.mask.clone(),
        );
        let pooled = self.encoder.pool(encoded, batch.mask.clone());
        let pooled = self.dropout.forward(pooled);
        self.classifier.forward(pooled) // [batch, NUM_OUTCOMES]
    }

    pub fn forward_classification(&self, batch: &OutcomeBatch<B>) -> ClassificationOutput<B> {
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

impl<B: AutodiffBackend> TrainStep<OutcomeBatch<B>, ClassificationOutput<B>>
    for OutcomePredictor<B>
{
    fn step(&self, batch: OutcomeBatch<B>) -> TrainOutput<ClassificationOutput<B>> {
        let output = self.forward_classification(&batch);
        TrainOutput::new(self, output.loss.backward(), output)
    }
}

impl<B: Backend> ValidStep<OutcomeBatch<B>, ClassificationOutput<B>> for OutcomePredictor<B> {
    fn step(&self, batch: OutcomeBatch<B>) -> ClassificationOutput<B> {
        self.forward_classification(&batch)
    }
}
