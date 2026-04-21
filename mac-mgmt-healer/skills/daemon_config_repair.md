---
name: Daemon Config Repair
desc: Fix daemon config.toml issues — ROCm on non-GPU hardware, disable unused services like LMS
---

Fix issues in the daemon's `config.toml` file, including incorrect GPU backend
configuration and disabling services that are not installed.

## Symptoms

- ollama UNHEALTHY: ROCm backend configured but no compatible GPU present.
- lms UNHEALTHY: "LM Studio daemon is not running and no valid installation could be found".
- ollama-env shows empty GPU device variables (CUDA_VISIBLE_DEVICES=, HIP_VISIBLE_DEVICES=).

## Procedure

1. **Read current config**
   - `read_file("daemon-config", "config.toml")` — note the mtime.
   - Look for `[ollama] flavour = "rocm"` or other GPU backend settings.
   - Look for presence/absence of `[lms]` section.

2. **Check hardware**
   - `get_inventory()` — check GPU fields.
   - `read_file("ollama-env")` — check if GPU vars are empty.
   - If no GPU hardware and `flavour = "rocm"`: this is the problem.

3. **Fix ROCm misconfiguration**
   - Remove `flavour = "rocm"` from the `[ollama]` section entirely (leaving `[ollama]`
     empty is fine — it defaults to CPU).
   - Do NOT set `flavour = "cpu"` — just remove the flavour line.

4. **Disable LMS if not installed**
   - If lms is crash-looping with "no valid installation":
   - Add `[lms]` section with `enabled = false`.

5. **Write config**
   - `write_file("daemon-config", "config.toml", <content>, expected_mtime)`.
   - The daemon picks up config.toml changes on the next health tick (no explicit
     restart needed).

6. **Verify**
   - `wait(30)` then `get_probe_status`.
   - For ollama: check that liveness/functional probes recover.
   - For lms: confirm it stops being spawned.

## Notes

- The `flavour` field controls which Ollama binary variant the daemon downloads
  and runs. Setting it to "rocm" on hardware without an AMD GPU causes Ollama
  to fail to initialize its compute backend.
- These two issues (ROCm + LMS) frequently co-occur on test instances because
  the default test config includes both.
