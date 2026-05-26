use std::path::Path;

use super::features::{
    EmbedderSample, IssueClassifierSample, MAX_SEQ_LEN, OutcomeSample, TOOL_HISTORY_LEN,
    ToolSelectorSample,
};
use super::tokenizer;
use burn::data::dataset::Dataset;

/// Dataset of tool selector samples loaded from JSONL.
#[derive(Debug, Clone)]
pub struct ToolSelectorDataset {
    samples: Vec<ToolSelectorSample>,
}

impl ToolSelectorDataset {
    pub fn from_jsonl(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let samples: Vec<ToolSelectorSample> = content
            .lines()
            .filter(|l| !l.is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { samples })
    }

    pub fn split(self, validation_fraction: f64) -> (Self, Self) {
        let split_idx = ((1.0 - validation_fraction) * self.samples.len() as f64) as usize;
        let (train, val) = self.samples.split_at(split_idx);
        (
            Self {
                samples: train.to_vec(),
            },
            Self {
                samples: val.to_vec(),
            },
        )
    }
}

impl Dataset<ToolSelectorSample> for ToolSelectorDataset {
    fn get(&self, index: usize) -> Option<ToolSelectorSample> {
        self.samples.get(index).cloned()
    }

    fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Dataset of outcome prediction samples loaded from JSONL.
#[derive(Debug, Clone)]
pub struct OutcomeDataset {
    samples: Vec<OutcomeSample>,
}

impl OutcomeDataset {
    pub fn from_jsonl(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let samples: Vec<OutcomeSample> = content
            .lines()
            .filter(|l| !l.is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { samples })
    }

    pub fn split(self, validation_fraction: f64) -> (Self, Self) {
        let split_idx = ((1.0 - validation_fraction) * self.samples.len() as f64) as usize;
        let (train, val) = self.samples.split_at(split_idx);
        (
            Self {
                samples: train.to_vec(),
            },
            Self {
                samples: val.to_vec(),
            },
        )
    }
}

impl Dataset<OutcomeSample> for OutcomeDataset {
    fn get(&self, index: usize) -> Option<OutcomeSample> {
        self.samples.get(index).cloned()
    }

    fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Padded + batched tensor representation for tool selector training.
#[derive(Debug, Clone)]
pub struct ToolSelectorBatch<B: burn::tensor::backend::Backend> {
    /// Context token IDs [batch_size, MAX_SEQ_LEN].
    pub context: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    /// Role IDs [batch_size, MAX_SEQ_LEN].
    pub roles: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    /// Attention mask [batch_size, MAX_SEQ_LEN] (1 = real token, 0 = padding).
    pub mask: burn::tensor::Tensor<B, 2, burn::tensor::Float>,
    /// Tool history [batch_size, TOOL_HISTORY_LEN].
    pub tool_history: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    /// Current phase [batch_size].
    pub phase: burn::tensor::Tensor<B, 1, burn::tensor::Int>,
    /// Target tool class [batch_size].
    pub targets: burn::tensor::Tensor<B, 1, burn::tensor::Int>,
}

/// Batcher for tool selector samples.
#[derive(Clone)]
pub struct ToolSelectorBatcher<B: burn::tensor::backend::Backend> {
    device: B::Device,
}

impl<B: burn::tensor::backend::Backend> ToolSelectorBatcher<B> {
    pub fn new(device: B::Device) -> Self {
        Self { device }
    }
}

impl<B: burn::tensor::backend::Backend>
    burn::data::dataloader::batcher::Batcher<ToolSelectorSample, ToolSelectorBatch<B>>
    for ToolSelectorBatcher<B>
{
    fn batch(&self, items: Vec<ToolSelectorSample>) -> ToolSelectorBatch<B> {
        let batch_size = items.len();

        let mut context_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut roles_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut mask_data = vec![0.0f32; batch_size * MAX_SEQ_LEN];
        let mut history_data = vec![tokenizer::NUM_TOOLS as i64; batch_size * TOOL_HISTORY_LEN];
        let mut phase_data = vec![0i64; batch_size];
        let mut target_data = vec![0i64; batch_size];

        for (i, sample) in items.iter().enumerate() {
            let len = sample.context_tokens.len().min(MAX_SEQ_LEN);
            for j in 0..len {
                context_data[i * MAX_SEQ_LEN + j] = sample.context_tokens[j] as i64;
                roles_data[i * MAX_SEQ_LEN + j] = sample.context_roles[j] as i64;
                mask_data[i * MAX_SEQ_LEN + j] = 1.0;
            }
            for (j, &h) in sample
                .tool_history
                .iter()
                .enumerate()
                .take(TOOL_HISTORY_LEN)
            {
                history_data[i * TOOL_HISTORY_LEN + j] = h as i64;
            }
            phase_data[i] = sample.phase as i64;
            target_data[i] = sample.target_tool as i64;
        }

        ToolSelectorBatch {
            context: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(context_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            roles: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(roles_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            mask: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(mask_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            tool_history: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(history_data, [batch_size, TOOL_HISTORY_LEN]),
                &self.device,
            ),
            phase: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(phase_data, [batch_size]),
                &self.device,
            ),
            targets: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(target_data, [batch_size]),
                &self.device,
            ),
        }
    }
}

/// Padded + batched tensor representation for outcome prediction.
#[derive(Debug, Clone)]
pub struct OutcomeBatch<B: burn::tensor::backend::Backend> {
    pub tokens: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    pub roles: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    pub mask: burn::tensor::Tensor<B, 2, burn::tensor::Float>,
    pub targets: burn::tensor::Tensor<B, 1, burn::tensor::Int>,
}

/// Batcher for outcome prediction samples.
#[derive(Clone)]
pub struct OutcomeBatcher<B: burn::tensor::backend::Backend> {
    device: B::Device,
}

impl<B: burn::tensor::backend::Backend> OutcomeBatcher<B> {
    pub fn new(device: B::Device) -> Self {
        Self { device }
    }
}

impl<B: burn::tensor::backend::Backend>
    burn::data::dataloader::batcher::Batcher<OutcomeSample, OutcomeBatch<B>> for OutcomeBatcher<B>
{
    fn batch(&self, items: Vec<OutcomeSample>) -> OutcomeBatch<B> {
        let batch_size = items.len();

        let mut tokens_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut roles_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut mask_data = vec![0.0f32; batch_size * MAX_SEQ_LEN];
        let mut target_data = vec![0i64; batch_size];

        for (i, sample) in items.iter().enumerate() {
            let len = sample.tokens.len().min(MAX_SEQ_LEN);
            for j in 0..len {
                tokens_data[i * MAX_SEQ_LEN + j] = sample.tokens[j] as i64;
                roles_data[i * MAX_SEQ_LEN + j] = sample.roles[j] as i64;
                mask_data[i * MAX_SEQ_LEN + j] = 1.0;
            }
            target_data[i] = sample.target as i64;
        }

        OutcomeBatch {
            tokens: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(tokens_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            roles: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(roles_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            mask: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(mask_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            targets: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(target_data, [batch_size]),
                &self.device,
            ),
        }
    }
}

// ── Issue Classifier Dataset ───────────────────────────────────────

/// Dataset of issue classifier samples loaded from JSONL.
#[derive(Debug, Clone)]
pub struct IssueClassifierDataset {
    samples: Vec<IssueClassifierSample>,
}

impl IssueClassifierDataset {
    pub fn from_jsonl(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let samples: Vec<IssueClassifierSample> = content
            .lines()
            .filter(|l| !l.is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { samples })
    }

    pub fn split(self, validation_fraction: f64) -> (Self, Self) {
        let split_idx = ((1.0 - validation_fraction) * self.samples.len() as f64) as usize;
        let (train, val) = self.samples.split_at(split_idx);
        (
            Self {
                samples: train.to_vec(),
            },
            Self {
                samples: val.to_vec(),
            },
        )
    }
}

impl Dataset<IssueClassifierSample> for IssueClassifierDataset {
    fn get(&self, index: usize) -> Option<IssueClassifierSample> {
        self.samples.get(index).cloned()
    }
    fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Padded + batched tensor representation for issue classification.
#[derive(Debug, Clone)]
pub struct IssueClassifierBatch<B: burn::tensor::backend::Backend> {
    pub tokens: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    pub roles: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    pub mask: burn::tensor::Tensor<B, 2, burn::tensor::Float>,
    pub targets: burn::tensor::Tensor<B, 1, burn::tensor::Int>,
}

#[derive(Clone)]
pub struct IssueClassifierBatcher<B: burn::tensor::backend::Backend> {
    device: B::Device,
}

impl<B: burn::tensor::backend::Backend> IssueClassifierBatcher<B> {
    pub fn new(device: B::Device) -> Self {
        Self { device }
    }
}

impl<B: burn::tensor::backend::Backend>
    burn::data::dataloader::batcher::Batcher<IssueClassifierSample, IssueClassifierBatch<B>>
    for IssueClassifierBatcher<B>
{
    fn batch(&self, items: Vec<IssueClassifierSample>) -> IssueClassifierBatch<B> {
        let batch_size = items.len();

        let mut tokens_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut roles_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut mask_data = vec![0.0f32; batch_size * MAX_SEQ_LEN];
        let mut target_data = vec![0i64; batch_size];

        for (i, sample) in items.iter().enumerate() {
            let len = sample.tokens.len().min(MAX_SEQ_LEN);
            for j in 0..len {
                tokens_data[i * MAX_SEQ_LEN + j] = sample.tokens[j] as i64;
                roles_data[i * MAX_SEQ_LEN + j] = sample.roles[j] as i64;
                mask_data[i * MAX_SEQ_LEN + j] = 1.0;
            }
            target_data[i] = sample.target as i64;
        }

        IssueClassifierBatch {
            tokens: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(tokens_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            roles: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(roles_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            mask: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(mask_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            targets: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(target_data, [batch_size]),
                &self.device,
            ),
        }
    }
}

// ── Embedder Dataset ───────────────────────────────────────────────

/// Dataset of embedder samples loaded from JSONL.
#[derive(Debug, Clone)]
pub struct EmbedderDataset {
    samples: Vec<EmbedderSample>,
}

impl EmbedderDataset {
    pub fn from_jsonl(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let samples: Vec<EmbedderSample> = content
            .lines()
            .filter(|l| !l.is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { samples })
    }

    pub fn split(self, validation_fraction: f64) -> (Self, Self) {
        let split_idx = ((1.0 - validation_fraction) * self.samples.len() as f64) as usize;
        let (train, val) = self.samples.split_at(split_idx);
        (
            Self {
                samples: train.to_vec(),
            },
            Self {
                samples: val.to_vec(),
            },
        )
    }
}

impl Dataset<EmbedderSample> for EmbedderDataset {
    fn get(&self, index: usize) -> Option<EmbedderSample> {
        self.samples.get(index).cloned()
    }
    fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Padded + batched tensor representation for the embedder.
#[derive(Debug, Clone)]
pub struct EmbedderBatch<B: burn::tensor::backend::Backend> {
    pub tokens: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    pub roles: burn::tensor::Tensor<B, 2, burn::tensor::Int>,
    pub mask: burn::tensor::Tensor<B, 2, burn::tensor::Float>,
    /// Labels for contrastive learning [batch].
    pub labels: burn::tensor::Tensor<B, 1, burn::tensor::Int>,
}

#[derive(Clone)]
pub struct EmbedderBatcher<B: burn::tensor::backend::Backend> {
    device: B::Device,
}

impl<B: burn::tensor::backend::Backend> EmbedderBatcher<B> {
    pub fn new(device: B::Device) -> Self {
        Self { device }
    }
}

impl<B: burn::tensor::backend::Backend>
    burn::data::dataloader::batcher::Batcher<EmbedderSample, EmbedderBatch<B>>
    for EmbedderBatcher<B>
{
    fn batch(&self, items: Vec<EmbedderSample>) -> EmbedderBatch<B> {
        let batch_size = items.len();

        let mut tokens_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut roles_data = vec![tokenizer::PAD as i64; batch_size * MAX_SEQ_LEN];
        let mut mask_data = vec![0.0f32; batch_size * MAX_SEQ_LEN];
        let mut label_data = vec![0i64; batch_size];

        for (i, sample) in items.iter().enumerate() {
            let len = sample.tokens.len().min(MAX_SEQ_LEN);
            for j in 0..len {
                tokens_data[i * MAX_SEQ_LEN + j] = sample.tokens[j] as i64;
                roles_data[i * MAX_SEQ_LEN + j] = sample.roles[j] as i64;
                mask_data[i * MAX_SEQ_LEN + j] = 1.0;
            }
            label_data[i] = sample.label as i64;
        }

        EmbedderBatch {
            tokens: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(tokens_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            roles: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(roles_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            mask: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(mask_data, [batch_size, MAX_SEQ_LEN]),
                &self.device,
            ),
            labels: burn::tensor::Tensor::from_data(
                burn::tensor::TensorData::new(label_data, [batch_size]),
                &self.device,
            ),
        }
    }
}
