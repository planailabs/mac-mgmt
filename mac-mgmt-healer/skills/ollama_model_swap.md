---
name: Ollama Model Swap
desc: Swap the active Ollama model — check capabilities, pull, update openclaw and cluster config
---

Swap the active Ollama model used by openclaw or other services. Use this when a model
lacks required capabilities (tool calling, context window size), is too large, or needs
replacing for any reason.

## Prerequisites

- Know the **requirement**: tool support, minimum context window, max size, or specific family.
- Have access to `ollama-list`, `ollama-show`, `ollama-pull` shell commands.
- Have access to `openclaw-config` file tunnel and `patch_config` tool.

## Procedure

1. **Inventory current state**
   - `run_command("ollama-list")` to see installed models with sizes.
   - `run_command("ollama-show", "<current_model>")` to check architecture, context length,
     parameter count, and supported capabilities.
   - `read_file("openclaw-config", "openclaw.json")` to see which model openclaw is using
     under `agents.defaults.model.primary`.
   - `get_config()` to see `ollama.default_model` and `ollama.models` list.

2. **Select a replacement model**
   - Compare installed models against requirements using `ollama-show` for each.
   - Key fields: `context length`, `parameters`, model `architecture`.
   - Tool calling support: models from qwen, phi4, llama3, and mistral families generally
     support tools. SmolLM2 models ≥1.7B support tools; 360M and 135M do not.
   - If no installed model fits, you'll need to pull one — check disk space first via
     `get_system_sample`.

3. **Pull new model if needed**
   - `run_command("ollama-pull", "<model:tag>")` — this can take minutes for large models.
   - After pull, verify with `run_command("ollama-show", "<model:tag>")`.

4. **Adjust context window if needed**
   - If the model's native context length is below the requirement, set
     `OLLAMA_CONTEXT_LENGTH=<desired>` in the ollama-env file tunnel.
   - Read current env: `read_file("ollama-env", "ollama-env")`.
   - Add/update the line, preserving all existing variables.
   - Write back with `write_file`.
   - Note: this sets the context window for ALL models served by this Ollama instance.

5. **Update openclaw configuration**
   - `read_file("openclaw-config", "openclaw.json")` — note the `mtime`.
   - Change `agents.defaults.model.primary` to `"ollama/<new_model>"`.
   - Ensure `models.providers.ollama.models` array includes the new model with correct
     `contextWindow` value.
   - `write_file("openclaw-config", "openclaw.json", <updated>, expected_mtime)`.

6. **Update cluster configuration**
   - `patch_config` with updated `ollama.default_model` and `ollama.models` list.

7. **Restart and verify**
   - `run_command("ollama-restart")` if env changed; otherwise skip.
   - `run_command("openclaw-restart")` if config changed.
   - `wait(30)` then `request_assessment`.
   - `wait(60)` then `get_probe_status` to confirm services healthy.

## Common pitfalls

- **409 Conflict on write_file**: The file was modified since your last read. Re-read to
  get the fresh mtime before retrying.
- **Validator rejection**: openclaw config has a JSON schema validator. Don't add unknown
  fields (e.g. `"tools": true` is not a valid openclaw model field).
- **smollm2:135m and smollm2:360m do NOT support tools** — don't use them when tool
  calling is required.
- **OLLAMA_CONTEXT_LENGTH is global** — it affects all models, not just the one you're swapping.
