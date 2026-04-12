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
3. Use **Skills** and **Bundles** to configure what software is deployed
4. Set up **Rollout Groups** to control deployment stages

## Key Concepts

### Clusters

A cluster represents a managed macOS or Linux device. Each cluster has a unique identifier and can be assigned skills, MCP servers, and configuration. See [Cluster Setup](/docs/cluster-setup) for setup instructions.

### Configuration

Each cluster has a JSON configuration that controls the daemon, LLM providers, notifications, and more. See the [Configuration Reference](/docs/configuration-reference) for all available settings.

### Skills and Bundles

Skills are Nix-packaged tools that can be deployed to clusters. Bundles group skills for convenient assignment. See [Skills and Bundles](/docs/skills-and-bundles) for details.

### Tokens and API

All programmatic access uses bearer tokens with three scope levels. See [Tokens and API](/docs/tokens-and-api) for authentication details.

### Rollouts

Rollouts let you stage daemon version updates across your fleet. See [Rollouts](/docs/rollouts) for the full workflow.

### Organizations

Organizations group clusters and users for multi-tenant management. See [Organizations](/docs/organizations) for details.

### Monitoring

The Fleet dashboard, Prometheus metrics, and Apprise notifications keep you informed. See [Fleet Monitoring](/docs/fleet-monitoring) for setup.
