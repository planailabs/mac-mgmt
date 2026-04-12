# Getting Started

Welcome to mac-mgmt! This guide will help you get up and running.

## Overview

mac-mgmt is a fleet management system for macOS devices. It provides:

- **Cluster management** — organize devices into clusters
- **Skill deployment** — push skills to devices
- **MCP server management** — configure Model Context Protocol servers
- **Rollout control** — staged rollouts with groups and scheduling

## Quick Start

1. Log in with your organization's SSO credentials
2. Navigate to **Clusters** to see your managed devices
3. Use **Skills** and **Bundles** to configure what software is deployed
4. Set up **Rollout Groups** to control deployment stages

## Key Concepts

### Clusters

A cluster represents a managed macOS or Linux device. Each cluster has a unique identifier
and can be assigned skills, MCP servers, and configuration.

### Skills

Skills are Nix-packaged tools or applications that can be deployed to clusters.
They are organized by slug and can have multiple release channels.

### Bundles

Bundles group multiple skills together for easier assignment to clusters.

### Rollout Groups

Rollout groups allow you to stage deployments across your fleet, ensuring
changes are tested on a subset of devices before wider rollout.
