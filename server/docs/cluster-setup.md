---
audience: user
---

# Cluster Setup

## Setting up OpenClaw standalone (not managed by the daemon)

- Install OpenClaw as provided on the OpenClaw website
- Set `global.agent_provider` to `none`

## Setting up OpenClaw via the daemon

- Set `global.agent_provider` to `openclaw` (default)
- Set `global.llm_provider` to a provider of your choice (default `ollama`)
  - You can use the cloud provider and configure the settings under the section `cloud` to use external cloud services for OpenClaw

## Setting up just the model provider

- Set `global.agent_provider` to `none`
- Set `global.llm_provider` to a provider of your choice (default `ollama`)
- Note: without an agent, `default_model` has no effect, it will only download and manage models and start the Ollama server

## Production-readying a cluster

- Enable `global.external_processes`
  - This enables daemon restarts without having to restart the associated services
- Set `relay.url` to `wss://relay.plan.ai` or your company relay if provided by us
- Add `https://relay.plan.ai/metrics` to your Prometheus monitoring
  - Use an organization token
