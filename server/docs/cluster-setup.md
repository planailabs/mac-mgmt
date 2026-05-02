---
audience: user
---

# Cluster Setup

## Setting up OpenClaw standalone (not managed by the daemon)

- Install OpenClaw as provided on the OpenClaw website
- Leave `openclaw.enabled` as `false` (default) or set `global.default_agent` to `none`

## Setting up OpenClaw via the daemon

- Set `openclaw.enabled` to `true` and `global.default_agent` to `openclaw`
- Set `global.default_llm` to a provider of your choice and enable it (e.g. `ollama.enabled = true`)
  - You can add cloud provider entries under the `cloud` list to use external cloud services for OpenClaw

## Setting up just the model provider

- Leave `openclaw.enabled` as `false` (default) or set `global.default_agent` to `none`
- Set `global.default_llm` to a provider of your choice and enable it (e.g. `ollama.enabled = true`)
- Note: without an agent, `default_model` has no effect, it will only download and manage models and start the Ollama server

## Production-readying a cluster

- Set `relay.url` to `wss://relay.plan.ai` or your company relay if provided by us
- Add `https://relay.plan.ai/metrics` to your Prometheus monitoring
  - Use an organization token
