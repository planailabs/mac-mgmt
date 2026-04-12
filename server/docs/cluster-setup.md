# Cluster Setup

## Setting up openclaw standalone (not managed by the daemon)

- install openclaw as provided on the openclaw website
- set global.agent_provider to none

## Setting up openclaw via the daemon

- set global.agent_provider to openclaw (default)
- set global.llm_provider to a provider of your choice (default ollama)
  - you can use the cloud provider and configure the settings under the section cloud to use external cloud services for openclaw

## Setting up just the model provider

- set global.agent_provider to none
- set global.llm_provider to a provider of your choice (default ollama)
- note: without an agent default_model has no effect, it will only download and manage models and start the ollama server

## Production-ready-ing a cluster

- Enable global.external_processes
  - This enables daemon restarts without having to restart the associated services
- Set relay.url to wss://relay.plan.ai or your company relay if provided by us
- Add https://relay.plan.ai/metrics to your prometheus monitoring
  - Use an organization token
