# mac-mgmt

A self-updating daemon that manages OpenClaw and Ollama installations on macOS and Linux, plus a management server with a web UI for multi-customer config delivery.

## Features

- **Service management** for OpenClaw and Ollama via a common `ManagedService` trait
- **Self-updating binary** with automatic version checks and in-place replacement
- **Health monitoring** with configurable intervals and automatic repair
- **Nix-based package management** using plan-ai's custom nixpkgs
- **Cross-platform** service installation (launchd on macOS, systemd on Linux)
- **Crash recovery** via Sentry reporting and self-update on panic
- **Embedded setup scripts** compiled into the binary for zero-dependency deployment
- **TOML configuration** with sane defaults
- **Remote config** — daemon can fetch base config from the management server, merging local overrides on top
- **Management server** — Dioxus fullstack web UI for customer/token/config CRUD, plus a Rocket API for daemon config delivery

## Architecture

The project is a Cargo workspace with two crates:

```
mac-mgmt/
  Cargo.toml          # workspace root
  daemon/             # the daemon binary (mac-mgmt)
  server/             # the management server (mac-mgmt-server)
```

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
2. If `[server]` section is present, fetch remote base config and merge local on top
3. Ensure services installed via nix
4. Run service setup (OpenClaw config, etc.)
5. Spawn service processes
6. Skip first health check per service (grace period for startup)
7. After first healthy check, run `post_start` hook (model pulls, etc.)

### Remote config

When the daemon has a `[server]` section in its local config:

1. Fetches `GET {url}/api/config` with `Authorization: Bearer {token}`
2. Parses the response as TOML (the base config)
3. Recursively merges local config on top (local values override remote)
4. If the server is unreachable, logs a warning and uses local-only config

This allows centralized config management — set defaults per customer on the server, and let individual machines override specific values locally.

### Upgrade strategy

Upgrades are checked by building a temporary nix profile copy and comparing store paths. When an upgrade is found:

1. The nix upgrade is installed immediately
2. Restart is **deferred** until the service is idle (no active sessions for OpenClaw, no loaded models for Ollama)
3. After restart, health check is skipped once and `post_start` re-runs

## Management Server

The server (`mac-mgmt-server`) is a Dioxus fullstack application that provides:

- **Web UI** (port 3000) — customer management dashboard with TailwindCSS
- **Daemon API** (port 8080) — Rocket-based REST API for config delivery

### Prerequisites

- PostgreSQL database
- [Dioxus CLI](https://dioxuslabs.com/) (`dx`)

### Running the server

From the repo root inside `nix develop` (which provides `dx`, `node`,
`cargo`, and the wasm toolchain):

```bash
# 1. Enter the dev shell
nix develop

# 2. Create config.toml from the example (edit as needed)
cp server/config.example.toml server/config.toml

# 3. (Optional) If you don't have Postgres, start one via docker compose.
#    The connection URL is already wired up in the example config.
docker compose up -d

# 4. Install JS deps and build the Tailwind stylesheet
#    (re-run npm run tailwind whenever styles change; or leave it running)
cd server && npm install && npm run tailwind

# 5. Start the Dioxus fullstack dev server
#    (web UI on :3000, daemon API on :8080, hot-reload enabled,
#     migrations run automatically on startup)
cd server && dx serve
```

For a production build, use `dx build --release` instead of `dx serve`.

Migrations run automatically on startup. The server creates three tables: `customers`, `tokens`, and `customer_configs`.

### Web UI routes

| Route | Description |
|-------|-------------|
| `/` | Customer list |
| `/customers/new` | Create a new customer |
| `/customers/:id` | Customer detail — manage tokens and config |

### Daemon API

| Endpoint | Auth | Description |
|----------|------|-------------|
| `GET /api/config` | Bearer token | Returns the latest TOML config for the authenticated customer |

Tokens are SHA-256 hashed in the database. The raw token is shown once at creation time in the web UI.

### Workflow

1. Create a customer in the web UI
2. Generate an API token (copy the raw token — it's shown only once)
3. Upload a TOML config for the customer
4. On the daemon machine, add to `~/.config/mac-mgmt/config.toml`:
   ```toml
   [server]
   url = "https://mgmt.example.com:8080"
   token = "the-raw-token"
   ```
5. The daemon fetches the remote config on startup and merges it with local overrides

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

If the file doesn't exist, all defaults are used. See [`daemon/config.example.toml`](daemon/config.example.toml)

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

### Server options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `url` | string | none | Management server URL (e.g. `https://mgmt.example.com:8080`) |
| `token` | string | none | API token for authentication |

Both must be set for remote config to be fetched. If either is missing, the daemon uses local-only config.

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

# Build the daemon
cargo build -p mac-mgmt

# Build all daemon targets (Linux + macOS)
cd daemon && bash build.sh

# Build the server (requires dx CLI, provided by dev shell)
cd server && dx build
```

### Build targets (daemon)

| Target | Tool | Binary name |
|--------|------|-------------|
| `x86_64-unknown-linux-musl` | `cargo build` | `mac-mgmt-x86_64-unknown-linux-musl` |
| `aarch64-apple-darwin` | `cargo zigbuild` | `mac-mgmt-aarch64-apple-darwin` |

### Deployment

```bash
# Quick test on remote server
bash test.sh [args]
```

## CI/CD

GitLab CI runs on the `trunk` branch:

1. **Build** - `nix develop -c bash daemon/build.sh` (produces `mac-mgmt.tar.gz`)
2. **Upload** - `nix develop -c bash daemon/upload.sh dev` (rsync to update server)

## Project Structure

```
daemon/
  src/
    main.rs                CLI entry point (clap)
    daemon.rs              Main loop, self-update logic
    config.rs              TOML config loading + remote config merge
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
  tests/
    integration/           Shell-based integration tests

server/
  src/
    main.rs                Starts Dioxus fullstack + Rocket API
    db.rs                  PostgreSQL connection (sqlx)
    models.rs              Customer, Token, CustomerConfig
    api/
      mod.rs               Rocket mount point
      auth.rs              Bearer token request guard
      routes.rs            GET /api/config
    web/
      mod.rs
      app.rs               Dioxus router + App root
      components/
        layout.rs          Nav shell (TailwindCSS)
        customer_list.rs   Customer table
        customer_form.rs   Create customer form
        customer_detail.rs Single customer view
        token_list.rs      Create/revoke tokens
        config_editor.rs   TOML config editor
  migrations/
    001_initial.sql        customers, tokens, customer_configs tables
  Dioxus.toml              Dioxus CLI config
  tailwind.config.js       TailwindCSS config
  input.css                TailwindCSS entry point
```
