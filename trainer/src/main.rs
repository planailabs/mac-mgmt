use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mac-mgmt-trainer", about = "Train models on healer session data")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Export healer sessions from the database into training-ready JSONL.
    Export {
        /// PostgreSQL connection URL.
        #[arg(long, env = "DATABASE_URL")]
        db_url: String,
        /// Output directory for exported data.
        #[arg(long, default_value = "./training-data")]
        output_dir: String,
        /// Minimum number of messages in a session to include.
        #[arg(long, default_value = "5")]
        min_messages: usize,
    },
    /// Train a model on exported session data.
    Train {
        /// Which model to train.
        #[arg(long, value_enum)]
        model: ModelType,
        /// Path to exported training data directory.
        #[arg(long, default_value = "./training-data")]
        data: String,
        /// Directory to save trained model artifacts.
        #[arg(long, default_value = "./models")]
        output: String,
        /// Batch size for training.
        #[arg(long, default_value = "32")]
        batch_size: usize,
        /// Number of training epochs.
        #[arg(long, default_value = "20")]
        epochs: usize,
        /// Learning rate.
        #[arg(long, default_value = "1e-3")]
        learning_rate: f64,
    },
    /// Evaluate a trained model on held-out data.
    Eval {
        /// Which model to evaluate.
        #[arg(long, value_enum)]
        model: ModelType,
        /// Path to model checkpoint.
        #[arg(long)]
        checkpoint: String,
        /// Path to exported training data directory.
        #[arg(long, default_value = "./training-data")]
        data: String,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ModelType {
    ToolSelector,
    OutcomePredictor,
    IssueClassifier,
    Embedder,
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mac_mgmt_trainer=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Export {
            db_url,
            output_dir,
            min_messages,
        } => {
            mac_mgmt_trainer::export::run_export(&db_url, &output_dir, min_messages).await?;
        }
        Command::Train {
            model,
            data,
            output,
            batch_size,
            epochs,
            learning_rate,
        } => {
            let config = mac_mgmt_trainer::training::TrainConfig {
                data_dir: data,
                output_dir: output,
                batch_size,
                epochs,
                learning_rate,
            };
            match model {
                ModelType::ToolSelector => {
                    mac_mgmt_trainer::training::train_tool_selector::<
                        burn::backend::Autodiff<burn::backend::NdArray>,
                    >(config)?;
                }
                ModelType::OutcomePredictor => {
                    mac_mgmt_trainer::training::train_outcome_predictor::<
                        burn::backend::Autodiff<burn::backend::NdArray>,
                    >(config)?;
                }
                ModelType::IssueClassifier => {
                    mac_mgmt_trainer::training::train_issue_classifier::<
                        burn::backend::Autodiff<burn::backend::NdArray>,
                    >(config)?;
                }
                ModelType::Embedder => {
                    mac_mgmt_trainer::training::train_embedder::<
                        burn::backend::Autodiff<burn::backend::NdArray>,
                    >(config)?;
                }
            }
        }
        Command::Eval {
            model,
            checkpoint,
            data,
        } => {
            match model {
                ModelType::ToolSelector => {
                    mac_mgmt_trainer::inference::eval_tool_selector::<burn::backend::NdArray>(
                        &checkpoint,
                        &data,
                    )?;
                }
                ModelType::OutcomePredictor => {
                    mac_mgmt_trainer::inference::eval_outcome_predictor::<burn::backend::NdArray>(
                        &checkpoint,
                        &data,
                    )?;
                }
                ModelType::IssueClassifier => {
                    mac_mgmt_trainer::inference::eval_issue_classifier::<burn::backend::NdArray>(
                        &checkpoint,
                        &data,
                    )?;
                }
                ModelType::Embedder => {
                    mac_mgmt_trainer::inference::eval_embedder::<burn::backend::NdArray>(
                        &checkpoint,
                        &data,
                    )?;
                }
            }
        }
    }

    Ok(())
}
