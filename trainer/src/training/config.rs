/// Training configuration passed from the CLI.
pub struct TrainConfig {
    pub data_dir: String,
    pub output_dir: String,
    pub batch_size: usize,
    pub epochs: usize,
    pub learning_rate: f64,
}
