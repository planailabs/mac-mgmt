use core::marker::PhantomData;

use burn::tensor::backend::Backend;
use burn::tensor::{Int, Tensor};
use burn::train::metric::state::{FormatOptions, NumericMetricState};
use burn::train::metric::{Adaptor, Metric, MetricEntry, MetricMetadata, Numeric};

/// Input for the Top-K accuracy metric.
pub struct TopKAccuracyInput<B: Backend> {
    pub outputs: Tensor<B, 2>,
    pub targets: Tensor<B, 1, Int>,
}

/// Top-K accuracy metric: measures whether the correct class is in the top K predictions.
pub struct TopKAccuracyMetric<B: Backend> {
    k: usize,
    state: NumericMetricState,
    _b: PhantomData<B>,
}

impl<B: Backend> TopKAccuracyMetric<B> {
    pub fn new(k: usize) -> Self {
        Self {
            k,
            state: NumericMetricState::new(),
            _b: PhantomData,
        }
    }
}

impl<B: Backend> Metric for TopKAccuracyMetric<B> {
    const NAME: &'static str = "Top-K Accuracy";
    type Input = TopKAccuracyInput<B>;

    fn update(&mut self, input: &Self::Input, _metadata: &MetricMetadata) -> MetricEntry {
        let [batch_size, num_classes] = input.outputs.dims();
        let k = self.k.min(num_classes);

        let logits_data = input.outputs.to_data();
        let targets_data = input.targets.to_data();
        let logits: &[f32] = logits_data.as_slice().unwrap();
        let targets: &[i64] = targets_data.as_slice().unwrap();

        let mut correct = 0usize;
        for i in 0..batch_size {
            let target = targets[i] as usize;
            let row = &logits[i * num_classes..(i + 1) * num_classes];

            let mut indexed: Vec<(usize, f32)> = row.iter().copied().enumerate().collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            if indexed.iter().take(k).any(|(idx, _)| *idx == target) {
                correct += 1;
            }
        }

        let accuracy = correct as f64 / batch_size as f64 * 100.0;
        self.state.update(
            accuracy,
            batch_size,
            FormatOptions::new(Self::NAME).unit("%").precision(2),
        )
    }

    fn clear(&mut self) {
        self.state = NumericMetricState::new();
    }
}

impl<B: Backend> Numeric for TopKAccuracyMetric<B> {
    fn value(&self) -> f64 {
        self.state.value()
    }
}

/// Adaptor: ClassificationOutput → TopKAccuracyInput.
impl<B: Backend> Adaptor<TopKAccuracyInput<B>> for burn::train::ClassificationOutput<B> {
    fn adapt(&self) -> TopKAccuracyInput<B> {
        TopKAccuracyInput {
            outputs: self.output.clone(),
            targets: self.targets.clone(),
        }
    }
}

/// Input for the MRR metric.
pub struct MrrInput<B: Backend> {
    pub outputs: Tensor<B, 2>,
    pub targets: Tensor<B, 1, Int>,
}

/// Mean Reciprocal Rank metric.
pub struct MrrMetric<B: Backend> {
    state: NumericMetricState,
    _b: PhantomData<B>,
}

impl<B: Backend> Default for MrrMetric<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: Backend> MrrMetric<B> {
    pub fn new() -> Self {
        Self {
            state: NumericMetricState::new(),
            _b: PhantomData,
        }
    }
}

impl<B: Backend> Metric for MrrMetric<B> {
    const NAME: &'static str = "MRR";
    type Input = MrrInput<B>;

    fn update(&mut self, input: &Self::Input, _metadata: &MetricMetadata) -> MetricEntry {
        let [batch_size, num_classes] = input.outputs.dims();

        let logits_data = input.outputs.to_data();
        let targets_data = input.targets.to_data();
        let logits: &[f32] = logits_data.as_slice().unwrap();
        let targets: &[i64] = targets_data.as_slice().unwrap();

        let mut rr_sum = 0.0f64;
        for i in 0..batch_size {
            let target = targets[i] as usize;
            let row = &logits[i * num_classes..(i + 1) * num_classes];

            let mut indexed: Vec<(usize, f32)> = row.iter().copied().enumerate().collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            if let Some(rank) = indexed.iter().position(|(idx, _)| *idx == target) {
                rr_sum += 1.0 / (rank as f64 + 1.0);
            }
        }

        let mrr = rr_sum / batch_size as f64;
        self.state
            .update(mrr, batch_size, FormatOptions::new(Self::NAME).precision(4))
    }

    fn clear(&mut self) {
        self.state = NumericMetricState::new();
    }
}

impl<B: Backend> Numeric for MrrMetric<B> {
    fn value(&self) -> f64 {
        self.state.value()
    }
}

/// Adaptor: ClassificationOutput → MrrInput.
impl<B: Backend> Adaptor<MrrInput<B>> for burn::train::ClassificationOutput<B> {
    fn adapt(&self) -> MrrInput<B> {
        MrrInput {
            outputs: self.output.clone(),
            targets: self.targets.clone(),
        }
    }
}
