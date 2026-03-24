# mac-mgmt

A self-updating daemon that manages OpenClaw and Ollama installations on macOS and Linux. Handles service installation, health monitoring, auto-repair, automatic upgrades, and crash recovery.

## Features

- **Service management** for OpenClaw and Ollama via a common `ManagedService` trait
- **Self-updating binary** with automatic version checks and in-place replacement
- **Health monitoring** with configurable intervals and automatic repair
- **Nix-based package management** using plan-ai's custom nixpkgs
- **Cross-platform** service installation (launchd on macOS, systemd on Linux)
- **Crash recovery** via Sentry reporting and self-update on panic
- **Embedded setup scripts** compiled into the binary for zero-dependency deployment
- **TOML configuration** with sane defaults

## Quick Start

```bash
# Initial setup: installs Nix, configures substituters, installs the service
mac-mgmt setup

# Or install the service manually
mac-mgmt install

# Check for updates manually
mac-mgmt update
mac-mgmt update --force
```

## CLI

```
mac-mgmt <COMMAND>

Commands:
  setup      Run the setup script and install the service
  run        Run an embedded script by name
  scripts    List all embedded scripts
  install    Install the launchd/systemd service
  uninstall  Remove the service
  restart    Restart the service
  daemon     Run the daemon (called by the service, not for manual use)
  update     Check for updates and apply (--force to skip version check)
```

## Daemon

The daemon manages two services (OpenClaw and Ollama) with two periodic loops:

| Loop | Interval | Actions |
|------|----------|---------|
| **Update** | 1 hour | Self-update check, nix upgrade check for managed services |
| **Health** | 1 minute | Process liveness, pending upgrade apply, health check + repair |

### Startup sequence

1. Load config from `~/.config/mac-mgmt/config.toml` (defaults if missing)
2. Ensure services installed via nix
3. Run service setup (OpenClaw config, etc.)
4. Spawn service processes
5. Skip first health check per service (grace period for startup)
6. After first healthy check, run `post_start` hook (model pulls, etc.)

### Upgrade strategy

Upgrades are checked by building a temporary nix profile copy and comparing store paths. When an upgrade is found:

1. The nix upgrade is installed immediately
2. Restart is **deferred** until the service is idle (no active sessions for OpenClaw, no loaded models for Ollama)
3. After restart, health check is skipped once and `post_start` re-runs

## Managed Services

### OpenClaw

| Operation | Implementation |
|-----------|---------------|
| Install | `nix profile add <nixpkgs>#openclaw` |
| Setup | `openclaw setup` if `~/.openclaw/openclaw.json` missing, then merge `extra_config` |
| Spawn | `openclaw gateway` |
| Health | `openclaw health --json` |
| Repair | `openclaw doctor --fix` |
| Busy check | `openclaw sessions --active 1 --json` (defers restart if sessions exist) |

### Ollama

| Operation | Implementation |
|-----------|---------------|
| Install | `nix profile add <nixpkgs>#ollama` |
| Spawn | `ollama serve` (with `OLLAMA_HOST` if non-default) |
| Health | HTTP `GET /` expecting `"Ollama is running"` |
| Post-start | `ollama pull` for each configured model, then `ollama launch --yes --config --model <default_model> openclaw` |
| Busy check | HTTP `GET /api/ps` (busy if models loaded in memory) |

## Configuration

Config file: `~/.config/mac-mgmt/config.toml`

If the file doesn't exist, all defaults are used. See [`config.example.toml`](config.example.toml):

```toml
[openclaw]
provider = "ollama"
# extra_config = { "key" = "value" }

[ollama]
host = "127.0.0.1"
port = 11434
models = ["qwen3.5", "qwen3-coder-next", "glm-5", "kimi-k2.5", "minimax-m2.7"]
default_model = "qwen3.5"
```

### OpenClaw options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `provider` | string | `"ollama"` | Backend provider name |
| `extra_config` | JSON object | none | Merged recursively into `~/.openclaw/openclaw.json` |

### Ollama options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `host` | string | `"127.0.0.1"` | Bind address |
| `port` | integer | `11434` | Listen port |
| `models` | string array | see above | Models to pull on post-start |
| `default_model` | string | `"qwen3.5"` | Model used for `ollama launch` |

## Self-Update

The daemon checks `https://update.plan.ai/{ENVIRONMENT}/mac-mgmt.version` hourly. If the remote version differs from the compiled version:

1. Downloads `mac-mgmt.tar.gz` from the update server
2. Extracts the binary matching the current target triple (`mac-mgmt-{TARGET}`)
3. Replaces itself in-place via `self-replace`

The `ENVIRONMENT` variable (default: `dev`) is embedded at compile time and determines the update channel.

## Service Installation

### macOS (launchd)

- Plist: `~/Library/LaunchAgents/com.plan-ai.mac-mgmt.plist`
- Runs at login, kept alive
- Logs: `/tmp/com.plan-ai.mac-mgmt.{out,err}.log`

### Linux (systemd)

- Unit: `/etc/systemd/system/mac-mgmt.service`
- System service running as the installing user
- Auto-restart on failure (5s delay)
- Runs via `bash -lc` for nix profile availability
- Uses `sudo` automatically when not root

## Building

### Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled

### Development

```bash
# Enter the dev shell
nix develop

# Build for current platform
cargo build

# Build all targets (Linux + macOS)
bash build.sh
```

### Build targets

| Target | Tool | Binary name |
|--------|------|-------------|
| `x86_64-unknown-linux-musl` | `cargo build` | `mac-mgmt-x86_64-unknown-linux-musl` |
| `aarch64-apple-darwin` | `cargo zigbuild` | `mac-mgmt-aarch64-apple-darwin` |

### Deployment

```bash
# Build and upload to update server (dev channel)
bash build.sh
bash upload.sh dev

# Quick test on remote server
bash test.sh [args]
```

## CI/CD

GitLab CI runs on the `trunk` branch:

1. **Build** - `nix develop -c bash build.sh` (produces `mac-mgmt.tar.gz`)
2. **Upload** - `nix develop -c bash upload.sh dev` (rsync to update server)

## Project Structure

```
src/
  main.rs                CLI entry point (clap)
  daemon.rs              Main loop, self-update logic
  config.rs              TOML config loading
  crash.rs               Sentry + panic recovery
  nix.rs                 Nix profile operations
  managed_service.rs     ManagedService trait
  scripts.rs             Embedded script runner (rust-embed)
  service/
    mod.rs               Platform dispatch (#[cfg] attributes)
    launchd.rs           macOS plist management
    systemd.rs           Linux systemd unit management
  services/
    mod.rs
    openclaw.rs          OpenClaw implementation + config
    ollama.rs            Ollama implementation + config
scripts/
  setup.sh               Nix installation and initial setup
```
