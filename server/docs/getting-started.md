---
audience: user
ordering_override: -100
---

# Getting Started

Welcome to mac-mgmt! This guide will help you get up and running.

## Overview

mac-mgmt is a fleet management system for macOS and Linux devices. It provides:

- **Cluster management** — organize devices into clusters with per-cluster configuration
- **Skill deployment** — push Nix-packaged skills to devices
- **MCP server management** — configure Model Context Protocol servers for OpenClaw
- **Rollout control** — staged rollouts with groups and scheduling
- **Fleet monitoring** — heartbeats, metrics, and notifications

## Quick Start

1. Log in with your organization's SSO credentials
2. Navigate to **Clusters** to see your managed devices
3. Open the **Fleet** dashboard to monitor device health
4. Check **Docs** for guides on specific topics

## Key Concepts

### Clusters

A cluster represents a managed macOS or Linux device. Each cluster has a unique identifier and can be assigned skills, MCP servers, and configuration. See [Cluster Setup](/docs/cluster-setup) for setup instructions.

### Configuration

Each cluster has a JSON configuration that controls the daemon, LLM providers, notifications, and more. See the [Configuration Reference](/docs/configuration-reference) for all available settings.

### Fleet Monitoring

The Fleet dashboard shows heartbeat status, daemon versions, and service health. See [Fleet Monitoring](/docs/fleet-monitoring) for details on metrics and notifications.
