//! Per-backend canary model identifiers.
//!
//! These are small, fast models used for functional probes and always
//! pulled/loaded during `post_start` so probes never fail due to a missing model.

pub const OLLAMA_CANARY: &str = "qwen3:0.6b";
pub const LMS_CANARY: &str = "lmstudio-community/Qwen3-0.6B-GGUF";
