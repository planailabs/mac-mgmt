---
name: Chat Completions Endpoint
desc: Enable the /v1/chat/completions endpoint on the openclaw gateway when disabled (404 on functional probe)
---

Enable the OpenAI-compatible chat completions endpoint when it's disabled, causing
functional probe failures with HTTP 404.

## Symptoms

- Functional probe returns 404 on `/v1/chat/completions`.
- Liveness probe passes (gateway is running and responding).
- `run_command("openclaw-health")` returns `"ok": true`.

## Procedure

1. **Confirm**
   - `get_probe_history(service: "openclaw")` — look for 404 on /v1/chat/completions.
   - `run_command("openclaw-health")` — should show ok: true (gateway alive).

2. **Read and fix config**
   - `read_file("openclaw-config", "openclaw.json")` — note the mtime.
   - Check `gateway.http.endpoints.chatCompletions.enabled`.
   - If missing or false, add/set it to `true`.

3. **Write and restart**
   - `write_file("openclaw-config", "openclaw.json", <updated>, expected_mtime)`.
   - `run_command("openclaw-restart")`.
   - Follow `probe_verification` skill.

## Notes

- The openclaw gateway does NOT enable chatCompletions by default.
- The functional probe specifically tests this endpoint, so it always fails if disabled.
- This is distinct from model issues — the 404 comes from the gateway routing layer,
  not from Ollama.
