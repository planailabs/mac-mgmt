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

Top-level settings that control which providers and features are active.

| Field | Default | Description |
|-------|---------|-------------|
| `llm_provider` | `"ollama"` | LLM backend: `ollama`, `nexa`, `lms`, `cloud`, or `none` |
| `agent_provider` | `"openclaw"` | Agent provider: `openclaw` or `none` |
| `agent_name` | *none* | Display name for this agent |
| `user_name` | *none* | Display name for the user |
| `external_processes` | `false` | Run managed services as independent system services (launchd/systemd) instead of child processes. Recommended for production |

## `notifications`

Configure where the daemon sends event notifications using [Apprise](https://github.com/caronc/apprise) URLs.

| Field | Default | Description |
|-------|---------|-------------|
| `urls` | `[]` | Apprise notification URLs (e.g. `tgram://bot/chat`, `ntfy://host/topic`) |
| `events` | *all* | Which events trigger notifications. Options: `daemon_started`, `daemon_stopped`, `service_crashed`, `service_unhealthy`, `service_recovered`, `upgrade_installed`, `upgrade_failed` |

## `ollama`

Settings for the Ollama local LLM server. Only applies when `global.llm_provider` is `"ollama"`.

| Field | Default | Description |
|-------|---------|-------------|
| `host` | `"127.0.0.1"` | Listen address |
| `port` | `11434` | Listen port |
| `models` | `["phi4-mini", "qwen3.5", "Flux_AI/Flux_AI"]` | Models to pull on startup; at least one required |
| `default_model` | `"phi4-mini"` | Default model for OpenClaw to use |
| `flavour` | `"cpu"` | Package flavour: `cpu`, `rocm` (AMD), `cuda` (NVIDIA), or `vulkan` |

## `nexa`

Settings for the Nexa local LLM server. Only applies when `global.llm_provider` is `"nexa"`.

| Field | Default | Description |
|-------|---------|-------------|
| `host` | `"127.0.0.1"` | Listen address |
| `port` | `18181` | Listen port |
| `models` | `["ggml-org/Qwen3-1.7B-GGUF"]` | Models to pull on startup; at least one required |
| `default_model` | `"ggml-org/Qwen3-1.7B-GGUF"` | Default model for OpenClaw to use |

## `lms`

Settings for LM Studio. Only applies when `global.llm_provider` is `"lms"`.

| Field | Default | Description |
|-------|---------|-------------|
| `host` | `"127.0.0.1"` | Listen address |
| `port` | `1234` | Listen port |
| `models` | `[]` | Model identifiers to load on startup via `lms load` |
| `default_model` | `"qwen2.5-coder-7b-instruct"` | Default model identifier for OpenClaw to use |

## `cloud`

Settings for cloud LLM providers. Only applies when `global.llm_provider` is `"cloud"`.

| Field | Default | Description |
|-------|---------|-------------|
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

Settings for the OpenClaw agent. Only applies when `global.agent_provider` is `"openclaw"`.

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
    "llm_provider": "ollama",
    "agent_provider": "openclaw",
    "external_processes": true
  },
  "ollama": {
    "models": ["phi4-mini", "qwen3.5"],
    "default_model": "phi4-mini",
    "flavour": "cpu"
  },
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
