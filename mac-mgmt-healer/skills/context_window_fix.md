---
name: Context Window Fix
desc: Fix openclaw functional probe failures caused by model context window below the 16,000 token minimum
---

Fix openclaw functional probe failures caused by the configured model having a
context window below OpenClaw's 16,000 token minimum.

## Symptoms

- Functional probe fails for openclaw (liveness may pass).
- Logs contain: `low context window: ollama/<model> ctx=NNNN (warn<32000) source=modelsConfig`
- The configured model's native context length is below 16,000 tokens.

## Quick reference: SmolLM2 model capabilities

| Model | Context | Tools | Size |
|-------|---------|-------|------|
| smollm2:135m | 8192 | NO | 270 MB |
| smollm2:360m | 8192 | NO | 725 MB |
| smollm2:1.7b | 8192 | YES | 1.8 GB |

**None of the SmolLM2 models meet the 16k context minimum natively.**

## Procedure

1. **Confirm the issue**
   - `fetch_logs(service: "openclaw")` — look for "low context window" message.
   - `run_command("ollama-show", "<current_model>")` — verify native context length.
   - `read_file("openclaw-config", "openclaw.json")` — check `agents.defaults.model.primary`
     and `models.providers.ollama.models[].contextWindow`.

2. **Choose a fix strategy**

   **Option A — Swap to a model with sufficient native context (preferred):**
   - `run_command("ollama-list")` to see what's installed.
   - `phi4-mini` has 131,072 context, supports tools, is 2.5 GB. Good default choice.
   - If already installed, skip the pull step.
   - If not installed: `run_command("ollama-pull", "phi4-mini")`.

   **Option B — Override context globally via OLLAMA_CONTEXT_LENGTH:**
   - Only use this when the user specifically wants to keep a small model.
   - Read current env: `read_file("ollama-env")` (note: this is a file tunnel, no sub-path).
   - Add/update `OLLAMA_CONTEXT_LENGTH=16384` (or higher).
   - Write back with `write_file`.
   - **Warning**: This is global — affects ALL models served by Ollama.
   - Requires `run_command("ollama-restart")`.

3. **Update openclaw.json**
   - `read_file("openclaw-config", "openclaw.json")` — note the `mtime`.
   - Change `agents.defaults.model.primary` to `"ollama/<new_model>"`.
   - Update the `models.providers.ollama.models` array: set `contextWindow` to match
     the actual value (use ollama-show output).
   - **Do NOT add `"tools": true`** — the validator rejects unknown fields.
   - `write_file("openclaw-config", "openclaw.json", <content>, expected_mtime)`.
   - If 409 Conflict: re-read and retry with fresh mtime.
   - If 422 Unprocessable Entity: fix JSON structure per error message.

4. **Update cluster config**
   - `patch_config` with updated `ollama.default_model` and `ollama.models` list.

5. **Restart and verify**
   - `run_command("openclaw-restart")` (or wait for daemon to pick up changes).
   - Follow the `probe_verification` skill: `request_assessment`, `wait(60)`,
     `get_probe_status`.

## Common pitfalls

- **Both configs must be updated**: `patch_config` updates cluster config but does NOT
  automatically update `openclaw.json`. You must write both.
- **smollm2:360m is NOT better than 135m for context** — both have 8192 tokens.
- **ollama-pull can timeout** for large models. Check `ollama-list` afterward.
- **Don't use send_push("sync_config") alone** — it syncs daemon config, not
  openclaw.json directly.
