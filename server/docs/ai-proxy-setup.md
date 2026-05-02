---
audience: user
---

# AI Proxy Setup

The AI proxy provides an OpenAI-compatible API (`/v1/chat/completions`, `/v1/models`) that sits in front of local LLM backends (Ollama, Unsloth). It handles authentication, per-key token budgets, and can load-balance requests across cluster peers when the relay is enabled.

## Enabling the proxy

Set `ai_proxy.enabled` to `true` in your cluster configuration and add at least one API key:

```json
{
  "ai_proxy": {
    "enabled": true,
    "keys": [
      {
        "name": "dev-team",
        "key_hash": "1220..."
      }
    ]
  }
}
```

The proxy listens on `127.0.0.1:18900` by default. Change `host` and `port` if needed.

## Generating API keys

API keys are stored as SHA2-256 multihashes — the server never sees or stores the raw key. To create a key:

1. Generate a random key (any string works):
   ```
   openssl rand -hex 32
   ```
   This gives you something like `a1b2c3d4...` — prefix it or use it as-is.

2. Compute the multihash. The format is `0x12` (SHA2-256 code) + `0x20` (32-byte length) + SHA2-256 digest, hex-encoded:
   ```
   echo -n "your-api-key" | python3 -c "
   import hashlib, sys
   d = hashlib.sha256(sys.stdin.buffer.read()).digest()
   print(bytes([0x12, 0x20]).hex() + d.hex())
   "
   ```

3. Add the hash to the cluster config:
   ```json
   {
     "name": "my-key",
     "key_hash": "1220<sha256-hex>"
   }
   ```

4. Save the raw key — it cannot be recovered from the hash.

## Token budgets

Each key can have a sliding-window token budget that limits total tokens (input + output) over a time period.

| Field | Default | Description |
|-------|---------|-------------|
| `token_budget` | `0` | Maximum tokens in the window. `0` = unlimited |
| `budget_window` | `"24h"` | Window duration (e.g. `"1h"`, `"7d"`) |

When a key exceeds its budget, requests return `429 Too Many Requests` until older usage falls outside the window. The budget is tracked per-key across all backends.

## Disabling a key

Set `enabled` to `false` on the key entry. Requests using that key will receive `403 Forbidden`.

## Available endpoints

All endpoints except `/health` require `Authorization: Bearer <raw-key>`.

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/chat/completions` | Chat completion (streaming and non-streaming) |
| GET | `/v1/models` | List models from all backends |
| GET | `/v1/usage` | Token usage stats for the authenticated key |
| GET | `/health` | Backend status (no auth required) |

## Backend routing

The proxy routes requests to whichever local backend is available:

1. If Ollama is enabled, it is preferred.
2. If only Unsloth is enabled, Unsloth is used.
3. If the relay is active and `ai_proxy_distribution` is enabled, the proxy compares local load against cluster peers and routes to the least-loaded node.

The `model` field in the request is passed through to the backend — the proxy does not filter by model name.

## Functional probes

When the proxy is enabled, the daemon automatically:

- Runs a **liveness probe** every ~60 seconds (`GET /health`).
- Runs a **functional probe** every ~30 minutes that sends a canary prompt (`qwen3:0.6b`) through `POST /v1/chat/completions` using an internal probe token. This validates the full pipeline from proxy through backend to model inference.

Probe results appear in the fleet dashboard under the instance's health status.

## Testing with curl

```bash
# Health check (no auth)
curl http://127.0.0.1:18900/health

# List models
curl http://127.0.0.1:18900/v1/models \
  -H "Authorization: Bearer your-api-key"

# Chat completion
curl http://127.0.0.1:18900/v1/chat/completions \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3:0.6b",
    "messages": [{"role": "user", "content": "Hello"}],
    "max_tokens": 32
  }'

# Check usage
curl http://127.0.0.1:18900/v1/usage \
  -H "Authorization: Bearer your-api-key"
```

## Exposing the proxy via relay

When the relay is configured, the proxy port is exposed as a TCP tunnel. Remote clients can connect through the relay without direct network access to the instance. The tunnel name is `ai-proxy` and appears in the instance detail page.

See [Configuration Reference](/docs/configuration-reference) for the full field list.
