use std::path::Path;
use std::process::Command as Process;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// Path to the Python finetune scripts, relative to the trainer crate root.
const FINETUNE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/finetune");

#[derive(Parser)]
#[command(
    name = "mac-mgmt-trainer",
    about = "Train models on healer session data"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Export healer sessions from the database into training-ready JSONL.
    Export {
        #[arg(long, env = "DATABASE_URL")]
        db_url: String,
        #[arg(long, default_value = "./training-data")]
        output_dir: String,
        #[arg(long, default_value = "5")]
        min_messages: usize,
    },

    /// Export sessions as ChatML conversations for SFT fine-tuning.
    ExportSft {
        /// Read sessions from DB.
        #[arg(long, env = "DATABASE_URL")]
        db_url: Option<String>,
        /// Or read from a previously exported training-data directory.
        #[arg(long)]
        input_dir: Option<String>,
        #[arg(long, default_value = "./sft-data")]
        output_dir: String,
        /// Path to a custom system prompt template file.
        #[arg(long)]
        system_prompt_template: Option<String>,
        /// Path to a trained embedder checkpoint for curriculum ordering.
        #[arg(long)]
        embedder_checkpoint: Option<String>,
        /// Enable data augmentation (tool perturbation, phase dropout).
        #[arg(long)]
        augment: bool,
        #[arg(long, default_value = "5")]
        min_messages: usize,
    },

    /// Train a Burn model on exported session data.
    Train {
        #[arg(long, value_enum)]
        model: ModelType,
        #[arg(long, default_value = "./training-data")]
        data: String,
        #[arg(long, default_value = "./models")]
        output: String,
        #[arg(long, default_value = "32")]
        batch_size: usize,
        #[arg(long, default_value = "20")]
        epochs: usize,
        #[arg(long, default_value = "1e-3")]
        learning_rate: f64,
        #[arg(long, value_enum, default_value = "cpu")]
        backend: BackendType,
    },

    /// Evaluate a trained Burn model on held-out data.
    Eval {
        #[arg(long, value_enum)]
        model: ModelType,
        #[arg(long)]
        checkpoint: String,
        #[arg(long, default_value = "./training-data")]
        data: String,
        #[arg(long, value_enum, default_value = "cpu")]
        backend: BackendType,
    },

    /// Fine-tune an LLM with QLoRA on SFT conversation data (calls Python).
    Finetune {
        /// Path to sft_conversations.jsonl.
        #[arg(long)]
        data: String,
        /// Output directory for model artifacts.
        #[arg(long, default_value = "./finetune-output")]
        output: String,
        /// Base model to fine-tune.
        #[arg(long)]
        base_model: Option<String>,
        /// Number of training epochs.
        #[arg(long)]
        epochs: Option<u32>,
        /// Learning rate.
        #[arg(long)]
        learning_rate: Option<f64>,
        /// LoRA rank.
        #[arg(long)]
        lora_rank: Option<u32>,
        /// Max sequence length.
        #[arg(long)]
        max_seq_length: Option<u32>,
    },

    /// Evaluate a fine-tuned LLM on held-out sessions (calls Python).
    EvalSft {
        /// Path to sft_conversations.jsonl.
        #[arg(long)]
        data: String,
        /// Evaluate via Ollama model name.
        #[arg(long)]
        ollama_model: Option<String>,
        /// Or evaluate via LoRA adapter directory.
        #[arg(long)]
        adapter_dir: Option<String>,
        /// Maximum eval samples.
        #[arg(long, default_value = "50")]
        max_samples: usize,
    },

    /// Export fine-tuned model to GGUF and register in Ollama (calls Python).
    ExportGguf {
        /// Path to LoRA adapter directory.
        #[arg(long)]
        adapter_dir: String,
        /// Output directory for GGUF file.
        #[arg(long)]
        output: Option<String>,
        /// GGUF quantization method (e.g. q5_k_m, q4_k_m).
        #[arg(long)]
        quantization: Option<String>,
        /// Ollama model name to register as.
        #[arg(long)]
        ollama_model_name: Option<String>,
        /// Skip Ollama registration.
        #[arg(long)]
        skip_ollama: bool,
    },

    /// Run the full fine-tuning pipeline: export → curriculum → SFT → train → GGUF → Ollama.
    Pipeline {
        /// PostgreSQL connection URL.
        #[arg(long, env = "DATABASE_URL")]
        db_url: String,
        /// Base model to fine-tune.
        #[arg(long, default_value = "Qwen/Qwen2.5-Coder-7B-Instruct")]
        base_model: String,
        /// Working directory for all artifacts.
        #[arg(long, default_value = "./pipeline-output")]
        work_dir: String,
        /// Enable data augmentation.
        #[arg(long)]
        augment: bool,
        /// Skip Ollama registration at the end.
        #[arg(long)]
        skip_ollama: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ModelType {
    ToolSelector,
    OutcomePredictor,
    IssueClassifier,
    Embedder,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum BackendType {
    Cpu,
    Gpu,
}

// ── Burn model train/eval dispatch ─────────────────────────────────

fn run_train(
    model: ModelType,
    config: mac_mgmt_trainer::training::TrainConfig,
    backend: BackendType,
) -> Result<()> {
    match (model, backend) {
        (ModelType::ToolSelector, BackendType::Cpu) => {
            mac_mgmt_trainer::training::train_tool_selector::<
                burn::backend::Autodiff<burn::backend::NdArray>,
            >(config)
        }
        (ModelType::ToolSelector, BackendType::Gpu) => {
            mac_mgmt_trainer::training::train_tool_selector::<
                burn::backend::Autodiff<burn::backend::wgpu::Wgpu>,
            >(config)
        }
        (ModelType::OutcomePredictor, BackendType::Cpu) => {
            mac_mgmt_trainer::training::train_outcome_predictor::<
                burn::backend::Autodiff<burn::backend::NdArray>,
            >(config)
        }
        (ModelType::OutcomePredictor, BackendType::Gpu) => {
            mac_mgmt_trainer::training::train_outcome_predictor::<
                burn::backend::Autodiff<burn::backend::wgpu::Wgpu>,
            >(config)
        }
        (ModelType::IssueClassifier, BackendType::Cpu) => {
            mac_mgmt_trainer::training::train_issue_classifier::<
                burn::backend::Autodiff<burn::backend::NdArray>,
            >(config)
        }
        (ModelType::IssueClassifier, BackendType::Gpu) => {
            mac_mgmt_trainer::training::train_issue_classifier::<
                burn::backend::Autodiff<burn::backend::wgpu::Wgpu>,
            >(config)
        }
        (ModelType::Embedder, BackendType::Cpu) => mac_mgmt_trainer::training::train_embedder::<
            burn::backend::Autodiff<burn::backend::NdArray>,
        >(config),
        (ModelType::Embedder, BackendType::Gpu) => mac_mgmt_trainer::training::train_embedder::<
            burn::backend::Autodiff<burn::backend::wgpu::Wgpu>,
        >(config),
    }
}

fn run_eval(model: ModelType, checkpoint: &str, data: &str, backend: BackendType) -> Result<()> {
    match (model, backend) {
        (ModelType::ToolSelector, BackendType::Cpu) => {
            mac_mgmt_trainer::inference::eval_tool_selector::<burn::backend::NdArray>(
                checkpoint, data,
            )
        }
        (ModelType::ToolSelector, BackendType::Gpu) => {
            mac_mgmt_trainer::inference::eval_tool_selector::<burn::backend::wgpu::Wgpu>(
                checkpoint, data,
            )
        }
        (ModelType::OutcomePredictor, BackendType::Cpu) => {
            mac_mgmt_trainer::inference::eval_outcome_predictor::<burn::backend::NdArray>(
                checkpoint, data,
            )
        }
        (ModelType::OutcomePredictor, BackendType::Gpu) => {
            mac_mgmt_trainer::inference::eval_outcome_predictor::<burn::backend::wgpu::Wgpu>(
                checkpoint, data,
            )
        }
        (ModelType::IssueClassifier, BackendType::Cpu) => {
            mac_mgmt_trainer::inference::eval_issue_classifier::<burn::backend::NdArray>(
                checkpoint, data,
            )
        }
        (ModelType::IssueClassifier, BackendType::Gpu) => {
            mac_mgmt_trainer::inference::eval_issue_classifier::<burn::backend::wgpu::Wgpu>(
                checkpoint, data,
            )
        }
        (ModelType::Embedder, BackendType::Cpu) => {
            mac_mgmt_trainer::inference::eval_embedder::<burn::backend::NdArray>(checkpoint, data)
        }
        (ModelType::Embedder, BackendType::Gpu) => mac_mgmt_trainer::inference::eval_embedder::<
            burn::backend::wgpu::Wgpu,
        >(checkpoint, data),
    }
}

// ── Python script helpers ──────────────────────────────────────────

/// Run a Python script from the finetune/ directory, streaming stdout/stderr.
fn run_python(script: &str, args: &[&str]) -> Result<()> {
    let script_path = Path::new(FINETUNE_DIR).join(script);
    if !script_path.exists() {
        anyhow::bail!(
            "Python script not found: {} (FINETUNE_DIR={})",
            script_path.display(),
            FINETUNE_DIR
        );
    }

    tracing::info!(
        "running: python {} {}",
        script_path.display(),
        args.join(" ")
    );

    let status = Process::new("python")
        .arg(&script_path)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("failed to execute Python — is it in PATH?")?;

    if !status.success() {
        anyhow::bail!("Python script failed with exit code: {}", status);
    }
    Ok(())
}

// ── Main ───────────────────────────────────────────────────────────

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
        // ── Burn model data export ─────────────────────────────────
        Cmd::Export {
            db_url,
            output_dir,
            min_messages,
        } => {
            mac_mgmt_trainer::export::run_export(&db_url, &output_dir, min_messages).await?;
        }

        // ── SFT data export ────────────────────────────────────────
        Cmd::ExportSft {
            db_url,
            input_dir,
            output_dir,
            system_prompt_template,
            embedder_checkpoint,
            augment,
            min_messages,
        } => {
            let template_content = system_prompt_template
                .as_ref()
                .map(std::fs::read_to_string)
                .transpose()
                .context("failed to read system prompt template")?;

            mac_mgmt_trainer::export::run_export_sft(
                db_url.as_deref(),
                input_dir.as_deref(),
                &output_dir,
                template_content.as_deref(),
                embedder_checkpoint.as_deref(),
                augment,
                min_messages,
            )
            .await?;
        }

        // ── Burn model training ────────────────────────────────────
        Cmd::Train {
            model,
            data,
            output,
            batch_size,
            epochs,
            learning_rate,
            backend,
        } => {
            let config = mac_mgmt_trainer::training::TrainConfig {
                data_dir: data,
                output_dir: output,
                batch_size,
                epochs,
                learning_rate,
            };
            run_train(model, config, backend)?;
        }

        // ── Burn model evaluation ──────────────────────────────────
        Cmd::Eval {
            model,
            checkpoint,
            data,
            backend,
        } => {
            run_eval(model, &checkpoint, &data, backend)?;
        }

        // ── LLM fine-tuning (Python) ───────────────────────────────
        Cmd::Finetune {
            data,
            output,
            base_model,
            epochs,
            learning_rate,
            lora_rank,
            max_seq_length,
        } => {
            let mut args = vec!["--data", &data, "--output", &output];
            let base_model_str;
            let epochs_str;
            let lr_str;
            let rank_str;
            let seq_str;

            if let Some(ref m) = base_model {
                base_model_str = m.clone();
                args.extend(["--base-model", &base_model_str]);
            }
            if let Some(e) = epochs {
                epochs_str = e.to_string();
                args.extend(["--epochs", &epochs_str]);
            }
            if let Some(lr) = learning_rate {
                lr_str = lr.to_string();
                args.extend(["--learning-rate", &lr_str]);
            }
            if let Some(r) = lora_rank {
                rank_str = r.to_string();
                args.extend(["--lora-rank", &rank_str]);
            }
            if let Some(s) = max_seq_length {
                seq_str = s.to_string();
                args.extend(["--max-seq-length", &seq_str]);
            }

            run_python("train.py", &args)?;
        }

        // ── LLM evaluation (Python) ────────────────────────────────
        Cmd::EvalSft {
            data,
            ollama_model,
            adapter_dir,
            max_samples,
        } => {
            let max_str = max_samples.to_string();
            let mut args = vec!["--data", &data, "--max-samples", &max_str];

            if let Some(ref m) = ollama_model {
                args.extend(["--ollama-model", m]);
            }
            if let Some(ref d) = adapter_dir {
                args.extend(["--adapter-dir", d]);
            }

            run_python("eval_generate.py", &args)?;
        }

        // ── GGUF export (Python) ───────────────────────────────────
        Cmd::ExportGguf {
            adapter_dir,
            output,
            quantization,
            ollama_model_name,
            skip_ollama,
        } => {
            let mut args = vec!["--adapter-dir", &adapter_dir];

            if let Some(ref o) = output {
                args.extend(["--output", o]);
            }
            if let Some(ref q) = quantization {
                args.extend(["--quantization", q]);
            }
            if let Some(ref n) = ollama_model_name {
                args.extend(["--ollama-model-name", n]);
            }
            if skip_ollama {
                args.push("--skip-ollama");
            }

            run_python("export_gguf.py", &args)?;
        }

        // ── Full pipeline ──────────────────────────────────────────
        Cmd::Pipeline {
            db_url,
            base_model,
            work_dir,
            augment,
            skip_ollama,
        } => {
            let work = Path::new(&work_dir);
            std::fs::create_dir_all(work)?;

            let raw_dir = work.join("raw-export");
            let sft_dir = work.join("sft-data");
            let models_dir = work.join("burn-models");
            let ft_dir = work.join("finetune-output");

            let raw_dir_s = raw_dir.to_string_lossy().to_string();
            let sft_dir_s = sft_dir.to_string_lossy().to_string();
            let models_dir_s = models_dir.to_string_lossy().to_string();
            let ft_dir_s = ft_dir.to_string_lossy().to_string();

            // Step 1: Export from DB
            tracing::info!("=== Step 1/6: Export sessions from DB ===");
            mac_mgmt_trainer::export::run_export(&db_url, &raw_dir_s, 5).await?;

            // Step 2: Train embedder (for curriculum)
            tracing::info!("=== Step 2/6: Train session embedder ===");
            let embedder_config = mac_mgmt_trainer::training::TrainConfig {
                data_dir: raw_dir_s.clone(),
                output_dir: models_dir_s.clone(),
                batch_size: 32,
                epochs: 10,
                learning_rate: 1e-3,
            };
            // Best-effort: if not enough data, curriculum falls back to heuristic
            if let Err(e) = mac_mgmt_trainer::training::train_embedder::<
                burn::backend::Autodiff<burn::backend::NdArray>,
            >(embedder_config)
            {
                tracing::warn!("embedder training failed (will use heuristic curriculum): {e}");
            }

            // Step 3: Export SFT data with curriculum + optional augmentation
            tracing::info!("=== Step 3/6: Export SFT conversations ===");
            let embedder_ckpt = models_dir.join("embedder_final");
            let embedder_ckpt_str = embedder_ckpt.to_string_lossy().to_string();
            let ckpt_exists = embedder_ckpt.with_extension("mpk").exists()
                || embedder_ckpt.with_extension("bin").exists()
                || embedder_ckpt.exists();

            mac_mgmt_trainer::export::run_export_sft(
                None,
                Some(&raw_dir_s),
                &sft_dir_s,
                None,
                if ckpt_exists {
                    Some(embedder_ckpt_str.as_str())
                } else {
                    None
                },
                augment,
                5,
            )
            .await?;

            // Step 4: Fine-tune LLM
            tracing::info!("=== Step 4/6: Fine-tune LLM with QLoRA ===");
            let sft_path = sft_dir.join("sft_conversations.jsonl");
            let sft_path_s = sft_path.to_string_lossy().to_string();
            run_python(
                "train.py",
                &[
                    "--data",
                    &sft_path_s,
                    "--output",
                    &ft_dir_s,
                    "--base-model",
                    &base_model,
                ],
            )?;

            // Step 5: Export GGUF
            tracing::info!("=== Step 5/6: Export to GGUF ===");
            let adapter_dir = ft_dir.join("lora_adapter");
            let adapter_dir_s = adapter_dir.to_string_lossy().to_string();
            let mut gguf_args = vec!["--adapter-dir", &adapter_dir_s, "--output", &ft_dir_s];
            if skip_ollama {
                gguf_args.push("--skip-ollama");
            }
            run_python("export_gguf.py", &gguf_args)?;

            // Step 6: Evaluate
            tracing::info!("=== Step 6/6: Evaluate ===");
            if !skip_ollama {
                run_python(
                    "eval_generate.py",
                    &["--data", &sft_path_s, "--ollama-model", "mac-mgmt-healer"],
                )?;
            } else {
                run_python(
                    "eval_generate.py",
                    &["--data", &sft_path_s, "--adapter-dir", &adapter_dir_s],
                )?;
            }

            tracing::info!("=== Pipeline complete! ===");
            tracing::info!("Artifacts in: {}", work_dir);
            if !skip_ollama {
                tracing::info!("Model registered as: mac-mgmt-healer");
                tracing::info!("Test with: ollama run mac-mgmt-healer");
            }
        }
    }

    Ok(())
}
