---
audience: user
---

# Configuration Reference

Each cluster has a JSON configuration managed through the web UI or the Setting API. This document describes every available section and field.

## `daemon`

Controls the daemon's own operational behavior.

| Field | Default | Description |
|-------|---------|-------------|
| `update_interval` | `"1h"` | How often to check for updates, sync skills and MCP servers (e.g. `"30s"`, `"5m"`, `"1h"`) |
| `health_interval` | `"1m"` | How often to run health checks on managed services |
| `log_level` | `"info"` | Log verbosity: `error`, `warn`, `info`, `debug`, or `trace` |
| `upgrade_window` | *none* | Time window for upgrades in `HH:MM-HH:MM` format (e.g. `"02:00-05:00"`). Omit to allow anytime |

## `global`

Top-level settings that control which default providers are active. Individual providers are toggled via their own `enabled` flag.

| Field | Default | Description |
|-------|---------|-------------|
| `default_llm` | `"ollama"` | Default LLM backend: `ollama`, `lms`, `cloud`, or `none` |
| `default_agent` | `"openclaw"` | Default agent provider: `openclaw`, `opencode`, or `none` |
| `agent_name` | *none* | Display name for this agent |
| `user_name` | *none* | Display name for the user |

## `notifications`

Configure where the daemon sends event notifications using [Apprise](https://github.com/caronc/apprise) URLs.

| Field | Default | Description |
|-------|---------|-------------|
| `urls` | `[]` | Apprise notification URLs (e.g. `tgram://bot/chat`, `ntfy://host/topic`) |
| `events` | *all* | Which events trigger notifications. Options: `daemon_started`, `daemon_stopped`, `service_crashed`, `service_unhealthy`, `service_recovered`, `upgrade_installed`, `upgrade_failed` |

## `ollama`

Settings for the Ollama local LLM server. Installed and started when `enabled` is `true`.

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Whether Ollama is installed and started |
| `host` | `"127.0.0.1"` | Listen address |
| `port` | `11434` | Listen port |
| `models` | `["phi4-mini", "qwen3.5", "Flux_AI/Flux_AI"]` | Models to pull on startup; at least one required |
| `default_model` | `"phi4-mini"` | Default model for OpenClaw to use |
| `flavour` | `"cpu"` | Package flavour: `cpu`, `rocm` (AMD), `cuda` (NVIDIA), or `vulkan` |
| `context_length` | `16384` | Context length passed as `OLLAMA_CONTEXT_LENGTH` env var |

## `lms`

Settings for LM Studio. Installed and started when `enabled` is `true`.

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Whether LM Studio is installed and started |
| `host` | `"127.0.0.1"` | Listen address |
| `port` | `1234` | Listen port |
| `models` | `[]` | Model identifiers to load on startup via `lms load` |
| `default_model` | `"qwen2.5-coder-7b-instruct"` | Default model identifier for OpenClaw to use |

## `cloud`

A list of cloud LLM provider entries. When `global.default_llm` is `"cloud"`, **all** enabled entries are configured simultaneously in the agent (OpenClaw or OpenCode). The first enabled entry's model is used as the default. This lets you set up multiple providers and switch between them from within the agent without reconfiguring the daemon.

Each entry has these fields:

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Whether this cloud provider entry is active |
| `provider` | `"anthropic"` | Cloud provider (see supported providers below) |
| `api_key` | *none* | API key for the provider |
| `default_model` | `"anthropic/claude-sonnet-4-6"` | Model identifier in `provider/model` format |
| `base_url` | *none* | Custom base URL for proxies or Bedrock |
| `api` | *auto* | API type override: `anthropic-messages`, `openai-completions`, `openai-responses`, `google-generative-ai`, `bedrock-converse-stream`, `ollama` |
| `auth` | *auto* | Authentication mode: `api-key`, `aws-sdk`, `oauth`, `token` |

### Supported cloud providers

| Provider | Default model | Environment variable |
|----------|---------------|---------------------|
| `anthropic` | `anthropic/claude-sonnet-4-6` | `ANTHROPIC_API_KEY` |
| `openai` | `openai/gpt-5.4` | `OPENAI_API_KEY` |
| `google` | `google/gemini-3-flash-preview` | `GEMINI_API_KEY` |
| `mistral` | `mistral/mistral-large-latest` | `MISTRAL_API_KEY` |
| `groq` | `groq/llama-4-scout-17b-16e-instruct` | `GROQ_API_KEY` |
| `xai` | `xai/grok-3-mini` | `XAI_API_KEY` |
| `deepseek` | `deepseek/deepseek-chat` | `DEEPSEEK_API_KEY` |
| `openrouter` | `openrouter/auto` | `OPENROUTER_API_KEY` |
| `together` | `together/meta-llama/Llama-4-Maverick-17B-128E-Instruct-Turbo` | `TOGETHER_API_KEY` |
| `bedrock` | `amazon-bedrock/us.anthropic.claude-sonnet-4-6-v1:0` | `AWS_ACCESS_KEY_ID` |

## `openclaw`

Settings for the OpenClaw agent. Installed and started when `enabled` is `true`.

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Whether OpenClaw is installed and started |

### `openclaw.gateway`

| Field | Default | Description |
|-------|---------|-------------|
| `port` | `18789` | Gateway listen port |
| `host` | `"127.0.0.1"` | Gateway listen address |

### `openclaw.skills`

| Field | Default | Description |
|-------|---------|-------------|
| `auto_update` | `true` | Automatically update skills on the update interval |

### `openclaw.telegram`

| Field | Default | Description |
|-------|---------|-------------|
| `bot_token` | *required* | Telegram bot token from @BotFather |
| `allowed_chat_ids` | `[]` | Allowed Telegram chat IDs. Empty means all chats are allowed |
| `enabled` | `true` | Enable the Telegram integration |

### `openclaw.extra_config`

Arbitrary key-value pairs merged into `~/.openclaw/openclaw.json` after the typed fields. Use this for OpenClaw settings not yet covered by the typed schema.

## `opencode`

Settings for the OpenCode agent. Installed and started when `enabled` is `true`.

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether OpenCode is installed and started |
| `port` | `18790` | Server listen port |
| `host` | `"127.0.0.1"` | Server listen address |

### `opencode.extra_config`

Arbitrary key-value pairs merged into the OpenCode config after typed fields. Use this for OpenCode settings not covered by the typed schema (e.g. `model`, `provider`).

## `metrics`

| Field | Default | Description |
|-------|---------|-------------|
| `port` | `9396` | Prometheus metrics endpoint port |

## `relay`

Settings for the p2p relay used for remote SSH, service tunnels, and cluster peer discovery.

| Field | Default | Description |
|-------|---------|-------------|
| `relay_multiaddr` | *none* | Relay node libp2p multiaddress (e.g. `"/dns4/relay.example.com/tcp/4001/wss"`). Also accepts `url` alias (e.g. `wss://relay.plan.ai`) |
| `remote_ssh_enabled` | `false` | Whether remote SSH access is enabled on startup |
| `tunnels_enabled` | `true` | Whether to expose service tunnels (TCP, file, shell) via the relay |
| `fake_origin_local` | `true` | Rewrite Host/Referer/Origin headers when proxying TCP tunnels. Prevents services like Ollama from rejecting requests with non-local origins |
| `cluster_psk` | *none* | Cluster pre-shared key for p2p peer authentication (hex-encoded, 32 bytes). Generated by the server |
| `mdns_enabled` | `true` | Enable mDNS local peer discovery on the LAN |
| `p2p_port` | `1122` | QUIC listen port for p2p connections |
| `ai_proxy_distribution` | `true` | Enable AI proxy request distribution across cluster peers. When enabled, the [AI proxy](/docs/ai-proxy-setup) load-balances to the least-loaded node |

## `healer`

Settings for the AI-powered auto-remediation agent. See [Healer](/docs/healer) for usage details.

| Field | Default | Description |
|-------|---------|-------------|
| `auto_trigger` | *none* | Enable auto-triggered healer sessions when a service stays unhealthy |
| `auto_trigger_provider` | *none* | Cloud provider for auto-triggered sessions (e.g. `"anthropic"`) |
| `auto_trigger_model` | *none* | Model for auto-triggered sessions (e.g. `"claude-sonnet-4-6"`) |
| `auto_approve` | *none* | Auto-approve remediation actions (skip the approval gate) |
| `fix_provider` | *none* | Provider for the fix-model used during the remediation phase |
| `fix_model` | *none* | Model for the remediation phase |

## `backup`

Restic-based backup configuration. When enabled, the daemon periodically backs up service data and configuration.

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether restic backups are enabled |
| `repository` | `""` | Restic repository path or URL (e.g. `/backup/restic`, `s3:bucket/prefix`, `sftp:host:/path`) |
| `password_file` | *none* | Path to the restic password/key file. Auto-generated if omitted |
| `interval` | `"6h"` | How often to run backups (e.g. `"6h"`, `"1d"`) |
| `keep_within` | `"7d"` | Retention policy: keep snapshots from the last N duration (e.g. `"7d"`, `"30d"`) |
| `extra_paths` | `[]` | Extra paths to include in backups beyond auto-detected service paths |
| `exclude` | `[]` | Glob patterns to exclude from backups |
| `include_models` | `false` | Whether to include large model caches (Ollama models, LM Studio cache) |
| `env` | `{}` | Environment variables for restic (e.g. `AWS_ACCESS_KEY_ID` for S3 backends) |

## `custom-service`

Define virtual services entirely through config. Each entry creates a service that can expose tunnels, file access, shell commands, probes, inventory, and security checks without writing code. Specified as a TOML array (`[[custom-service]]`) or JSON array.

| Field | Default | Description |
|-------|---------|-------------|
| `name` | *required* | Service name used as the identifier in heartbeats, tunnels, etc. |
| `enabled` | `true` | Whether this custom service is active |
| `spawn` | *none* | Process to spawn and supervise (see below). If absent, the service is integrated (no process) |
| `health_check` | *none* | Health check command. Exit code 0 = healthy |
| `tunnels` | `[]` | TCP ports to expose through the relay |
| `files` | `[]` | Files or folders exposed for remote editing |
| `commands` | `[]` | Predefined shell commands exposed for remote execution |
| `probes` | `[]` | HTTP or exec probes for functional health checks |
| `inventory` | `[]` | Static inventory entries collected periodically |
| `samples` | `[]` | Dynamic sample entries collected on each heartbeat |
| `security` | `[]` | Security checks (exec or file-based) |

### `custom-service.spawn`

| Field | Default | Description |
|-------|---------|-------------|
| `command` | *required* | Program to execute (e.g. `"grafana-server"`) |
| `args` | `[]` | Fixed arguments passed to the command |
| `env` | `{}` | Environment variables set for the spawned process |

### `custom-service.tunnels[]`

| Field | Default | Description |
|-------|---------|-------------|
| `name` | *required* | Short, URL-safe name (e.g. `"grafana"`) |
| `host` | `"127.0.0.1"` | Host the service listens on |
| `port` | *required* | TCP port number |

## `ai_proxy`

An OpenAI-compatible API proxy that sits in front of local LLM backends (Ollama, Unsloth) and optionally load-balances across cluster peers. See [AI Proxy Setup](/docs/ai-proxy-setup) for a detailed walkthrough.

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether the AI proxy is started |
| `host` | `"127.0.0.1"` | Listen address |
| `port` | `18900` | Listen port |
| `keys` | `[]` | API keys with per-key token budgets (see below) |

### `ai_proxy.keys[]`

| Field | Default | Description |
|-------|---------|-------------|
| `name` | `""` | Human-readable label for this key |
| `key_hash` | *required* | Hex-encoded SHA2-256 multihash of the API key |
| `token_budget` | `0` | Maximum total tokens (input+output) within the budget window. `0` = unlimited |
| `budget_window` | `"24h"` | Sliding window duration (e.g. `"24h"`, `"7d"`, `"1h"`) |
| `enabled` | `true` | Whether this key is active |

## `memvault`

Distributed p2p memory store for AI context sharing across cluster nodes. Requires the daemon to be compiled with `--features memvault`. See [Memvault Setup](/docs/memvault-setup) for a detailed walkthrough.

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether memvault is started |
| `data_dir` | `"~/.local/share/memvault"` | Data directory for storage (redb, identity) |
| `cluster_id` | `""` | Base58-encoded 32-byte cluster ID, or `"auto"` to generate on first run |
| `bootstrap_peers` | `[]` | libp2p multiaddrs for Kademlia bootstrap + initial connections. Include the `/p2p/<peer-id>` suffix so the peer is seeded into the DHT routing table before Identify completes |
| `port` | `8401` | API server port (bearer token auto-generated in `data_dir/api.token`) |
| `kad_server` | `false` | Force the Kademlia DHT into server mode. When `false`, libp2p auto-detects mode from confirmed external addresses — which can leave a NATed node stuck as a client and undiscoverable via the DHT. Set `true` on publicly-reachable nodes (relays/bootstrap) |
| `kad_bootstrap_interval_secs` | `0` | Seconds between Kademlia bootstrap rounds (a self-lookup that refreshes/expands the routing table). `0` disables periodic bootstrap; when `> 0` an initial bootstrap also runs at startup |

## Example configuration

```json
{
  "daemon": {
    "update_interval": "5m",
    "log_level": "info",
    "upgrade_window": "02:00-05:00"
  },
  "global": {
    "default_llm": "ollama",
    "default_agent": "openclaw"
  },
  "ollama": {
    "enabled": true,
    "models": ["phi4-mini", "qwen3.5"],
    "default_model": "phi4-mini",
    "flavour": "cpu"
  },
  "openclaw": {
    "enabled": true
  },
  "cloud": [
    {
      "enabled": true,
      "provider": "anthropic",
      "api_key": "sk-ant-...",
      "default_model": "anthropic/claude-sonnet-4-6"
    },
    {
      "enabled": true,
      "provider": "openai",
      "api_key": "sk-...",
      "default_model": "openai/gpt-5.4"
    }
  ],
  "notifications": {
    "urls": ["ntfy://ntfy.example.com/alerts"]
  },
  "relay": {
    "relay_multiaddr": "wss://relay.plan.ai",
    "remote_ssh_enabled": true,
    "tunnels_enabled": true,
    "mdns_enabled": true
  },
  "healer": {
    "auto_trigger": true,
    "auto_trigger_provider": "anthropic",
    "auto_trigger_model": "claude-sonnet-4-6"
  },
  "backup": {
    "enabled": true,
    "repository": "/backup/restic",
    "interval": "6h",
    "keep_within": "7d"
  },
  "ai_proxy": {
    "enabled": true,
    "keys": [
      {
        "name": "dev-team",
        "key_hash": "1220...",
        "token_budget": 1000000,
        "budget_window": "24h"
      }
    ]
  },
  "memvault": {
    "enabled": true,
    "port": 8401
  },
  "metrics": {
    "port": 9396
  }
}
```
