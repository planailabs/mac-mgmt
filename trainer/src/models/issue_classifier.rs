use burn::config::Config;
use burn::module::Module;
use burn::nn::{Dropout, DropoutConfig, Linear, LinearConfig};
use burn::tensor::backend::{AutodiffBackend, Backend};
use burn::tensor::Tensor;
use burn::train::{TrainOutput, TrainStep, ValidStep};

use crate::export::dataset::IssueClassifierBatch;

use super::common::{SessionEncoder, SessionEncoderConfig};

/// Number of ping categories: hardware, network, disk_space, config_error, service_crash,
/// model_issue, permission, dependency, security, performance, tool_needed, other.
pub const NUM_CATEGORIES: usize = 12;

/// Category names in the same order as classification indices.
pub const CATEGORY_NAMES: &[&str] = &[
    "hardware",
    "network",
    "disk_space",
    "config_error",
    "service_crash",
    "model_issue",
    "permission",
    "dependency",
    "security",
    "performance",
    "tool_needed",
    "other",
];

/// Configuration for the issue classifier model.
#[derive(Config, Debug)]
pub struct IssueClassifierConfig {
    pub encoder: SessionEncoderConfig,
}

/// Classification output for multi-label + single-label combined loss.
#[derive(Debug)]
pub struct IssueClassifierOutput<B: Backend> {
    pub loss: Tensor<B, 1>,
    /// Category logits [batch, NUM_CATEGORIES].
    pub category_logits: Tensor<B, 2>,
    /// Category targets [batch] (single-label index).
    pub category_targets: Tensor<B, 1, burn::tensor::Int>,
}

/// Predicts staff_ping category from initial issues + early tool results.
///
/// Uses a single-label classification head for the primary category,
/// since each staff_ping has exactly one category.
#[derive(Module, Debug)]
pub struct IssueClassifier<B: Backend> {
    encoder: SessionEncoder<B>,
    category_head: Linear<B>,
    dropout: Dropout,
}

impl IssueClassifierConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> IssueClassifier<B> {
        let d_model = self.encoder.d_model;
        let encoder = self.encoder.init(device);
        let category_head = LinearConfig::new(d_model, NUM_CATEGORIES).init(device);
        let dropout = DropoutConfig::new(self.encoder.dropout).init();

        IssueClassifier {
            encoder,
            category_head,
            dropout,
        }
    }
}

impl<B: Backend> IssueClassifier<B> {
    pub fn forward(&self, batch: &IssueClassifierBatch<B>) -> Tensor<B, 2> {
        let encoded =
            self.encoder
                .forward(batch.tokens.clone(), batch.roles.clone(), batch.mask.clone());
        let pooled = self.encoder.pool(encoded, batch.mask.clone());
        let pooled = self.dropout.forward(pooled);
        self.category_head.forward(pooled) // [batch, NUM_CATEGORIES]
    }

    pub fn forward_classification(
        &self,
        batch: &IssueClassifierBatch<B>,
    ) -> IssueClassifierOutput<B> {
        let category_logits = self.forward(batch);
        let targets = batch.targets.clone();
        let loss = burn::nn::loss::CrossEntropyLossConfig::new()
            .init(&category_logits.device())
            .forward(category_logits.clone(), targets.clone());

        IssueClassifierOutput {
            loss,
            category_logits,
            category_targets: targets,
        }
    }
}

/// Wrap into ClassificationOutput for Burn's built-in metrics.
impl<B: AutodiffBackend> TrainStep<IssueClassifierBatch<B>, burn::train::ClassificationOutput<B>>
    for IssueClassifier<B>
{
    fn step(
        &self,
        batch: IssueClassifierBatch<B>,
    ) -> TrainOutput<burn::train::ClassificationOutput<B>> {
        let out = self.forward_classification(&batch);
        let classification = burn::train::ClassificationOutput {
            loss: out.loss.clone(),
            output: out.category_logits,
            targets: out.category_targets,
        };
        TrainOutput::new(self, out.loss.backward(), classification)
    }
}

impl<B: Backend> ValidStep<IssueClassifierBatch<B>, burn::train::ClassificationOutput<B>>
    for IssueClassifier<B>
{
    fn step(&self, batch: IssueClassifierBatch<B>) -> burn::train::ClassificationOutput<B> {
        let out = self.forward_classification(&batch);
        burn::train::ClassificationOutput {
            loss: out.loss,
            output: out.category_logits,
            targets: out.category_targets,
        }
    }
}
