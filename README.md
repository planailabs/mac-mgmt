# mac-mgmt

A fleet management system for AI infrastructure. Manages service installations, configuration, health monitoring, and automated remediation across clusters of Linux and macOS machines.

## Features

- **Service management** — OpenClaw, Ollama, LM Studio, OpenCode, LiteLLM, Hermes, Apprise, McPorter, Restic and more via a common `ManagedService` trait
- **GPU support** — NVIDIA (CUDA/nvidia-smi) and AMD (ROCm/rocm-smi) GPU monitoring and driver management
- **Self-updating daemon** with automatic version checks and health-gated deploys (staged rollouts are driven by the server's rollout system)
- **Health monitoring** — three-tier probes (liveness, functional, regression) with configurable intervals
- **Nix-based package management** using plan-ai's custom nixpkgs with builtin file validators
- **SSH relay** — reverse SSH tunnels through a central relay for NAT-traversed remote access
- **File/shell/log tunnels** — browser-based config editing, command execution, and log tailing through the relay
- **Self-healing agent** — AI-powered automated diagnostics and repair via the healer subsystem
- **Staff pings** — actionable admin notifications from the healer agent with categories and resolution tracking
- **Multi-tenant** — organizations, clusters, RBAC with OIDC authentication
- **Remote config** — centralized config with local overrides, skills, MCP servers, and bundles
- **Rollout system** — staged deployments with health gates and auto-pause
- **Simulation testing** — Antithesis-style chaos and fault injection test framework

## Architecture

The project is a Cargo workspace (see [`DESIGN.md`](DESIGN.md) for the system design).
The main crates:

```
mac-mgmt/
  common/              # Shared types, config structs, wire formats (+ plan-ai-* sub-crates)
  mac-mgmt-agent/      # Reusable agent control plane (assessment, services, relay, config)
  daemon/              # The daemon binary (mac-mgmt)
  server/              # Management server (Dioxus fullstack + Rocket API)
  relay/               # SSH relay and HTTP tunnel proxy (axum)
  relay-ssh/           # CLI SSH client into relay-managed machines
  mac-mgmt-ws/         # Shared WebSocket utilities
  mac-mgmt-services/   # Service supervisor (in-process or socket)
  mac-mgmt-healer/     # Self-healing AI agent (swiftide + native tools)
  runner/              # Incus test-fleet orchestrator daemon
  mmr-causality/       # mmrc CLI + mmrcd chaos orchestrator
  sim-tests/           # Simulation test framework
```

`memvault/` and `common/` are git submodules; `design` is a symlink to the nested
`memvault/plan-ai-design` submodule, kept for Cargo lockfile path identity. Run
`git submodule update --init --recursive` before building.

## Quick Start

```bash
# Enter the dev shell (provides dx, cargo, node, overmind, wasm toolchain)
nix develop

# Copy example configs
./setup-configs.sh

# Install JS deps (first time only)
cd server && npm install && cd ..

# Start everything
overmind start
```

| Process | Description | Port |
|---------|-------------|------|
| **server** | Dioxus fullstack + Rocket API | Web: 7377, API: 7378 |
| **relay** | SSH relay + HTTP tunnel proxy | 7379 |
| **tailwind** | TailwindCSS watcher | — |

Migrations run automatically on startup. Use `DEV_ONLY_NO_AUTH=1` to bypass OIDC in development.

## Daemon

The daemon manages services with two periodic loops:

| Loop | Interval | Actions |
|------|----------|---------|
| **Update** | 1 hour | Self-update check, nix upgrade check |
| **Health** | 1 minute | Process liveness, pending upgrade apply, health probes |

### Assessment system

Three-tier health assessment:

| Tier | Cadence | Purpose |
|------|---------|---------|
| **Sample** | Every heartbeat | CPU, memory, disk, network, GPU metrics |
| **Inventory** | ~6 hours | Static system facts, security audit |
| **Probes** | ~15 minutes | End-to-end functional validation per service |

Probe results are reported via heartbeats (`services_extended`) and dedicated probe endpoints. Stale probes from disabled services are automatically cleaned up.

### Managed services

| Service | Package | Health Check | GPU |
|---------|---------|-------------|-----|
| **OpenClaw** | `openclaw` | Gateway health endpoint | — |
| **Ollama** | `ollama` / `ollama-rocm` / `ollama-cuda` | HTTP GET `/` | ROCm, CUDA |
| **LM Studio** | `lmstudio` | `lms server status --json` | — |
| **OpenCode** | `opencode` | Functional probe | — |
| **LiteLLM** | `litellm` | HTTP health endpoint | — |
| **Hermes** | `hermes` | Functional probe | — |
| **Apprise** | `apprise` | Liveness check | — |
| **McPorter** | `mcporter` | Liveness check | — |
| **Restic** | `restic` | Liveness check | — |
| **NVIDIA SMI** | `cudatoolkit` | GPU metrics collection | CUDA |
| **ROCm SMI** | `rocm-smi` | GPU metrics collection | ROCm |

The full set (including Memvault, the relay, the AI proxy and `[[custom-service]]`
entries) is assembled by `connectors::build_services` in `mac-mgmt-agent`.

### File tunnel validators

File tunnels support both builtin and external validators. Builtin validators run in-process (no binary dependency); external commands run as a fallback when available:

| Builtin | Validates |
|---------|-----------|
| `json` | JSON syntax via serde_json |
| `toml` | TOML syntax via toml crate |

When both `builtin` and `command` are set, both run — builtin first, then external. If the external binary is missing but builtin passed, the write is accepted with a warning.

## Management Server

The server provides:

- **Web UI** — Dioxus fullstack dashboard with TailwindCSS
- **REST API** — Rocket-based API for daemon sync, settings management, admin operations
- **SSE push** — real-time push notifications to connected daemons
- **OpenAPI/Swagger** — auto-generated API docs at `/api/swagger-ui`

### Key web UI pages

| Route | Description |
|-------|-------------|
| `/` | Cluster list |
| `/fleet` | Fleet dashboard — all instances with health badges |
| `/fleet/:id` | Instance detail — probes, services, tunnels |
| `/fleet/:id/files` | Config file editor (via file tunnels) |
| `/fleet/:id/shell` | Shell command execution (via shell tunnels) |
| `/fleet/:id/logs` | Live log viewer |
| `/fleet/:id/healer` | Self-healing agent — chat UI with session history |
| `/staff-pings` | Admin staff notifications from healer (resolvable) |
| `/rollouts` | Staged deployment management |
| `/skills` | Skill management |
| `/mcp-servers` | MCP server management |

### Auth model

| Token kind | Scope | Use case |
|------------|-------|----------|
| `sync` | Single cluster | Daemon polling (config, heartbeat, probes) |
| `setting` | Cluster or org | Config/skills/MCP management |
| `admin` | All clusters | Full access, cluster/rollout management |
| `proxy` | Cluster, 6h TTL | Browser tunnel access (file/shell/log) |

Web UI uses OIDC (Google, etc.) with org-based RBAC.

## Self-Healing Agent (mac-mgmt-healer)

AI-powered automated diagnostics and repair for customer servers. Runs as part of the server process, spawned on-demand via web UI or API.

### How it works

1. Healthcheck detects an issue (service unhealthy, probe failure)
2. Admin or automation triggers a healer session for the instance
3. The agent reads logs, configs, and system metrics via relay tunnels
4. Diagnoses the root cause and applies targeted fixes
5. Verifies the fix worked via health probes
6. Pins a diagnosis summary and final report for staff review

### LLM backend

Tries local Ollama first (zero cost), falls back to Anthropic cloud. Configurable via `[healer]` section in server config:

```toml
[healer]
ollama_url = "http://localhost:11434"
ollama_model = "gemma4"
anthropic_model = "claude-sonnet-4-6"
token_budget = 200000  # auto-pause per cloud session
```

### Agent tools

**Instance interaction**: list_files, read_file, write_file, run_command, fetch_logs, list_file_tunnels, list_shell_commands, fetch_cluster_logs, run_cluster_command

**Diagnostics**: get_inventory, get_system_sample, get_probe_status, get_probe_history, get_metrics, get_version_info, get_heartbeat, get_cluster_instances, get_service_state, nix_check_upgrades

**Session management**: pin (diagnosis/remediation/final_report), staff_ping, set_phase, check_node_online, wait_for_node, wait, request_assessment, send_push

**Knowledge**: read_doc, list_docs, list_builtin_skills, use_skill

**Cluster config**: get_config, patch_config, set_config, list_skills, add_skill, remove_skill, list_mcp_servers, add_mcp_server, remove_mcp_server

### Session state machine

```
Created → Initializing → Diagnosing → Remediating → Verifying → Done
                                                               → NeedsHumanAttention
                                                               → Failed
                                   (any active) → AwaitingRetry (auto-resume)
                                   (any active) → Paused (token budget, manual resume)
                                   (any active) → Cancelled
```

Sessions persist in PostgreSQL with full conversation history. Interrupted sessions auto-resume on server restart. Token budget auto-pauses cloud sessions to prevent runaway costs. Admins can extend budgets for paused sessions via a "More Tokens" button in the UI.

Export session transcripts for analysis:

```bash
mac-mgmt-server dump-sessions [-o healer-sessions]
```

### Built-in skills

Embedded remediation skills (`mac-mgmt-healer/skills/`) provide step-by-step procedures:

| Skill | Purpose |
|-------|---------|
| `ollama_model_swap` | Swap active Ollama model, handle sizing and capabilities |
| `service_crash_recovery` | Diagnose crashes, crash loops, port conflicts |
| `resource_triage` | Diagnose thermal, memory, disk, and GPU issues |
| `openclaw_config_repair` | Repair OpenClaw configuration issues |
| `daemon_config_repair` | Repair daemon configuration issues |
| `connectivity_diagnostics` | Diagnose network and connectivity problems |
| `cluster_comparison` | Compare configs and logs across cluster instances |
| `probe_verification` | Verify health probes are working correctly |
| `context_window_fix` | Fix context-window / model sizing problems |
| `chat_completions_endpoint` | Diagnose OpenAI-compatible endpoint issues |
| `file_tunnel_usage` | Use file tunnels correctly |

### Staff pings

The agent creates actionable notifications for admins with categories: hardware, network, disk_space, config_error, service_crash, model_issue, permission, dependency, security, performance, other. Pings are resolvable via the web UI.

### Daemon reconnect guard

All relay requests automatically wait up to 10 minutes if the daemon disconnects (502/503/504), then retry. The agent also has `check_node_online` and `wait_for_node` tools for explicit control.

## Relay

The relay (`mac-mgmt-relay`) provides:

- **Reverse SSH tunnels** — daemons connect outbound, relay assigns ports
- **HTTP proxy** — subdomain-routed TCP tunnel access (`{instance}.relay.example.com`)
- **File tunnels** — read/write config files on daemons
- **Shell tunnels** — execute predefined commands on daemons (with per-command timeouts)
- **Virtual commands** — `service-restart`, `restart-daemon`, `sync-state` and `memctl` without spawning processes
- **System commands** — built-in diagnostics: `df`, `nix-profile-list`, `nix-collect-garbage`, `daemon-status`, `daemon-journal`, `service-journal`, `systemctl-status`
- **Log proxy** — fetch daemon logs
- **WebSocket bridging** — SSH, file, and shell sessions over WebSocket

Port reservations persist across daemon reconnects (30-day TTL).

## Configuration

### Daemon config

File: `~/.config/mac-mgmt/config.toml` — see [`daemon/config.example.toml`](daemon/config.example.toml)

### Server config

File: `./config.toml` — see [`server/config.example.toml`](server/config.example.toml)

## Building

```bash
nix develop

# Daemon
cargo build -p mac-mgmt

# Server (requires dx CLI)
cd server && dx build
```

Nix builds the daemon through `dx build --embed @client … @server …` (see
`overlay.nix`), so a plain `cargo build --workspace` is not the release path.

## CI/CD

GitLab CI on `trunk` (`.gitlab-ci.yml`):

1. **Build** — `bash prebuild.sh` (`nix-image` runner).
2. **Test** — `nix develop -c bash build-memvault.sh` then
   `cargo test --workspace --locked --exclude mac-mgmt-executor`
   (excluded: its tests need a working `incus`).
3. **Deploy** — `nix flake check -L --max-jobs 1`, then `xzar.sh` and
   `docker-push.sh`; `usb-release.sh` publishes the sovereign-AI USB tarballs.
