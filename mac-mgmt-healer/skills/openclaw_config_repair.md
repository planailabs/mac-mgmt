---
name: OpenClaw Config Repair
desc: Fix openclaw.json configuration issues causing probe failures or incorrect behavior
---

Fix openclaw.json configuration issues that cause functional probe failures or
incorrect behavior.

## When to use

- Functional probe fails (HTTP 404, wrong model, missing endpoint).
- Telegram bot or other channels report LLM errors.
- Model mismatch between what's configured and what's actually running.

## Procedure

1. **Read current config**
   - `read_file("openclaw-config", "openclaw.json")` — note the `mtime` for writes.
   - Key sections:
     - `agents.defaults.model.primary` — the default LLM (format: `"ollama/<model>"`)
     - `models.providers.ollama.models` — declared model list with context windows
     - `channels` — telegram, web, etc.
     - `gateway.http.endpoints` — which API endpoints are enabled

2. **Cross-reference with reality**
   - `run_command("ollama-list")` — what models are actually installed?
   - `run_command("ollama-show", "<model>")` — actual context window and capabilities.
   - `get_config()` — what does the cluster config say the default model should be?

3. **Common issues and fixes**

   **Model mismatch**: `agents.defaults.model.primary` points to a model that isn't installed
   or has different capabilities than expected.
   → Update to match an actually installed, capable model.

   **Missing endpoint**: Functional probe hits `/v1/chat/completions` but the gateway has it
   disabled. Look for `gateway.http.endpoints.chatCompletions.enabled`.
   → Set to `true` if the probe expects it.

   **Wrong context window**: `models.providers.ollama.models[].contextWindow` doesn't match
   the actual model capability, causing openclaw to reject requests.
   → Update to match `ollama-show` output.

   **Invalid JSON structure**: Validator rejects writes with unknown fields.
   → Only use fields that exist in the current config. Don't add `"tools": true` or other
   invented fields to model entries.

4. **Apply fix**
   - Build the corrected JSON.
   - `write_file("openclaw-config", "openclaw.json", <content>, expected_mtime)`.
   - If you get 409 Conflict, re-read the file and retry with the new mtime.
   - If you get 422 Unprocessable Entity, the validator rejected your change — read the
     error message carefully and fix the JSON structure.

5. **Validate and restart**
   - `run_command("openclaw-config-validate")` to check the config is valid.
   - `run_command("openclaw-restart")` or wait for the daemon to pick up changes.
   - `wait(30)` then verify with `run_command("openclaw-health")`.

6. **Update cluster config if needed**
   - If you changed the default model, update `ollama.default_model` via `patch_config`.
   - Keep cluster config and local openclaw config in sync.
