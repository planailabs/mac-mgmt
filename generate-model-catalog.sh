#!/usr/bin/env bash
# Regenerate server/ext/model-catalog.json from live provider APIs.
#
# API keys are read from environment variables:
#   ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY,
#   MISTRAL_API_KEY, GROQ_API_KEY, XAI_API_KEY,
#   DEEPSEEK_API_KEY, TOGETHER_API_KEY
#
# Ollama and OpenRouter are public and always fetched.
# Providers without a key are silently skipped.
#
# Usage:
#   ./generate-model-catalog.sh            # default output
#   ./generate-model-catalog.sh --no-openrouter   # skip OpenRouter
#   RUST_LOG=mac_mgmt_server=debug ./generate-model-catalog.sh  # verbose

set -euo pipefail
cd "$(dirname "$0")"

cargo run -p mac-mgmt-server -- generate-model-catalog \
  --output server/ext/model-catalog.json \
  "$@"
