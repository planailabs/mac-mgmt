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

A list of cloud LLM provider entries. When `global.default_llm` is `"cloud"`, the first enabled entry is used. Multiple entries allow configuring several cloud providers at once.

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
| `port` | `8080` | Gateway listen port |
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

Settings for the relay server used for remote SSH access.

| Field | Default | Description |
|-------|---------|-------------|
| `url` | *none* | Relay server URL (e.g. `wss://relay.plan.ai`). Must start with `ws://` or `wss://` |
| `remote_ssh_enabled` | `false` | Whether remote SSH access is enabled on startup |

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
    }
  ],
  "notifications": {
    "urls": ["ntfy://ntfy.example.com/alerts"]
  },
  "relay": {
    "url": "wss://relay.plan.ai",
    "remote_ssh_enabled": true
  },
  "metrics": {
    "port": 9396
  }
}
```
