---
audience: admin
---

# Fleet Runner

The **mac-mgmt-runner** is an orchestrator daemon that spins up a matrix of test clusters on a remote [Incus](https://linuxcontainers.org/incus/) host, reconciles their configurations against the mac-mgmt server, monitors heartbeats, and continuously exercises the fleet with chaos operations.

It runs as a long-lived daemon with a local HTTP API. The same binary also exposes CLI subcommands that drive the daemon.

## What it does

For every cell in the **(agent provider) × (LLM provider) × (cluster size)** matrix the runner:

1. Creates a cluster on the mac-mgmt server, scoped to one organization.
2. Uploads the matching cluster config via the setting API.
3. Generates a fresh Ed25519 host key and derives the expected `instance_id` before any VM boots.
4. Fetches a ready-to-use cloud-init YAML from the server (via `POST /api/setting/cloud-init`), embedding the host key so the daemon reports the predicted `instance_id`.
5. Launches an Incus container/VM with the cloud-init as user-data.
6. Watches for a heartbeat matching the expected `instance_id`. If none arrives within `deploy_timeout` (default 15 minutes) the instance is torn down and recreated with a new host key — retrying indefinitely.
7. Runs two chaos loops: a **resource-churn loop** (install/uninstall random skills, bundles, MCP servers, MCP bundles every `chaos_interval`) and a **VM-chaos loop** (toggle instance power state or reprovision every `vm_chaos_interval`).

Clusters are automatically enrolled in a `{name_prefix}fleet` [rollout group](/docs/rollouts) for easy targeting.

## Prerequisites

| Requirement | Detail |
|---|---|
| mac-mgmt server | Reachable at its **REST API URL** (default port 7378), **not** the web UI port |
| Admin token | `kind = "admin"` — create via the web UI or directly in the database |
| Organization | All matrix clusters are created inside one configured organization |
| Incus host | HTTPS listener enabled, runner's client certificate trusted |
| NixOS (optional) | A NixOS module is provided for systemd deployment |

## Setup

### 1. Incus host

Enable the HTTPS API on the Incus host and trust the runner's client certificate:

```
incus config set core.https_address :8443
incus config trust add-certificate /path/to/runner-client.crt
```

### 2. Configuration

Copy the example config to `/etc/mac-mgmt-runner/config.toml` (or any path — override with `--config` or `MAC_MGMT_RUNNER_CONFIG`).

**Required fields:**

| Section | Key | Description |
|---|---|---|
| `[mgmt]` | `url` | REST API URL of the mac-mgmt server (port 7378, **not** the web UI) |
| `[mgmt]` | `admin_token` | Admin bearer token |
| `[mgmt]` | `organization_id` | UUID of the organization to create clusters under |
| `[incus]` | `url` | Incus HTTPS endpoint (e.g. `https://incus.example:8443`) |
| `[incus]` | `client_cert` | Path to PEM client certificate |
| `[incus]` | `client_key` | Path to PEM client key |

**Optional mgmt fields:**

| Key | Default | Description |
|---|---|---|
| `public_url` | server's `api.external_url` | URL baked into cloud-init for daemons to dial |
| `system` | `x86_64-linux` | Nix system identifier for the daemon download URL |
| `daemon_version` | auto-resolved | Pin a specific daemon version; omit to resolve from rollout/pinned/latest |

### 3. Start the daemon

```
mac-mgmt-runner --config /etc/mac-mgmt-runner/config.toml daemon
```

A preflight `GET /api/self` runs at startup. If the URL redirects to an OIDC sign-in page (web UI port), the runner refuses to start with a clear error.

## Configuration reference

### `[incus]`

| Key | Default | Description |
|---|---|---|
| `project` | `"default"` | Incus project to create instances in |
| `image_alias` | `"ubuntu/24.04/cloud"` | Image alias or fingerprint |
| `image_server` | `"https://images.linuxcontainers.org"` | Image server for remote lookups |
| `instance_type` | `"container"` | `"container"` or `"virtual-machine"` |
| `profiles` | `["default"]` | Incus profiles to attach |
| `name_prefix` | `"mmr-"` | Prefix for cluster names and instance names |
| `server_ca` | *(none)* | PEM CA for validating the Incus server; omit to skip TLS verification |

### `[api]`

| Key | Default | Description |
|---|---|---|
| `bind` | `"127.0.0.1"` | Bind address for the runner's local HTTP API |
| `port` | `9400` | Bind port |

### `[fleet]`

| Key | Default | Description |
|---|---|---|
| `reconcile_interval` | `"1m"` | How often the reconcile loop ticks |
| `vm_chaos_interval` | `"30m"` | Interval between VM-level chaos ops (toggle / reprovision) |
| `chaos_interval` | `"5m"` | Interval between resource-churn chaos ops (skills, bundles, MCP). `"0"` or `"off"` to disable |
| `state_path` | `"/var/lib/mac-mgmt-runner/state.json"` | Path to the persisted fleet state file |
| `startup_grace` | `"3m"` | Instances younger than this are not considered unhealthy |
| `heartbeat_stale_after` | `"5m"` | Heartbeat age threshold for marking an instance unhealthy |
| `deploy_timeout` | `"15m"` | Time a fresh instance has to produce a heartbeat before teardown + retry |
| `max_concurrent_launches` | `3` | Max cells in the Launching stage at once; others wait at ConfigPushed |

### `[sentry]`

| Key | Default | Description |
|---|---|---|
| `dsn` | *(none)* | Sentry DSN for error reporting. Omit to disable. |
| `environment` | *(none)* | Environment tag (e.g. `"staging"`, `"production"`) |

### `[matrix]`

| Key | Default | Description |
|---|---|---|
| `ollama_model` | `"smollm2:1.7b"` | Model for every Ollama-LLM cell (must support tools) |
| `lms_model` | `"smollm2-1.7b-instruct"` | Model for every LM Studio cell |
| `agents` | `["openclaw", "none"]` | Agent providers to include |
| `llms` | `["ollama", "lms", "cloud"]` | LLM providers to include |
| `cloud_providers` | all known | Cloud sub-providers to include (only those with API keys emit cells) |
| `cluster_sizes` | `[1, 2]` | Node counts per cluster; each size multiplies the matrix |
| `cloud_api_keys` | `{}` | Map of cloud provider → API key. Cells are only emitted for providers listed here. |

## Cell lifecycle (state machine)

Each matrix cell progresses through these stages:

```
Pending → ClusterCreated → ConfigPushed → Launching → Running
                                            ↓ timeout
                                     ConfigPushed (retry with new host key)
```

- **Pending** — nothing created yet
- **ClusterCreated** — cluster exists on the mgmt server
- **ConfigPushed** — cluster config uploaded
- **Launching** — Incus instances created, waiting for heartbeats
- **Running** — all expected heartbeats received

State is persisted to `state_path` after every transition. A crash at any point resumes from the recorded stage.

## Chaos operations

### Resource churn (every `chaos_interval`)

Picks a random running cluster and a random resource type (skill, bundle, MCP server, MCP bundle). Uses weighted randomness: `P(install) = not_installed / total`, `P(uninstall) = installed / total`. This keeps clusters around half-installed regardless of catalog size.

### VM chaos (every `vm_chaos_interval`)

Picks a random running cell and either:
- **Toggle** (90%) — queries live Incus state, stops a running instance or starts a stopped one
- **Reprovision** (10%) — destroys and rebuilds the entire cell

Instances stopped by chaos are automatically restarted after 2 hours to prevent prolonged outages.

## CLI commands

| Command | Description |
|---|---|
| `daemon` | Run the orchestrator daemon + HTTP API |
| `status [--json]` | Fleet snapshot (dials the running daemon) |
| `provision` | Force a full reconcile |
| `teardown --yes` | Destroy every provisioned cell and pause the runner |
| `redeploy --yes` | Wipe everything (including orphans on the server) and rebuild |
| `reprovision [KEY]` | Destroy + recreate one cell (random if KEY omitted) |
| `gc` | Delete untracked Incus instances and mgmt clusters matching the prefix |
| `chaos` | Fire one resource-churn tick manually |
| `matrix [--json]` | Print the computed matrix without touching anything |

After `teardown`, the runner pauses — automatic loops sit idle until `provision`, `reprovision`, or `redeploy` is called.

## HTTP API & dashboard

The runner serves a dashboard at `GET /` (default `http://127.0.0.1:9400/`) showing the fleet summary, a per-cell table, and action buttons. Auto-refreshes every 5 seconds.

| Endpoint | Description |
|---|---|
| `GET /` | HTML dashboard |
| `GET /status` | JSON snapshot |
| `POST /provision` | Trigger reconcile |
| `POST /teardown` | Destroy all cells + pause |
| `POST /redeploy` | Wipe + rebuild |
| `POST /gc` | Garbage-collect orphans |
| `POST /chaos` | Fire one resource-churn tick |
| `POST /reprovision` | Reprovision random cell |
| `POST /reprovision/<key>` | Reprovision specific cell |
| `POST /shutdown` | Graceful shutdown |

No auth on the HTTP API — bind to `127.0.0.1` and expose via SSH or firewall rules.

## NixOS deployment

The runner ships a NixOS module at `nixosModules.runner`:

```nix
{
  services.mac-mgmt-runner = {
    enable = true;
    package = mac-mgmt-runner;
    settings = {
      mgmt = {
        url = "https://mgmt.example.com:7378";
        admin_token = "<ADMIN_TOKEN>";
        organization_id = "00000000-0000-0000-0000-000000000000";
      };
      incus = {
        url = "https://incus.example:8443";
        client_cert = "/var/lib/mac-mgmt-runner/incus-client.crt";
        client_key  = "/var/lib/mac-mgmt-runner/incus-client.key";
      };
      matrix.ollama_model = "smollm2:1.7b";
    };
    environmentFile = "/run/secrets/mac-mgmt-runner.env";
  };
}
```

The module creates a dedicated user/group, a state directory under `/var/lib/mac-mgmt-runner`, and a hardened systemd unit. Secrets can be passed via `environmentFile`.

## Cloud-init endpoint

The server's `POST /api/setting/cloud-init` endpoint is usable beyond the runner — any automation that needs to bootstrap a fresh machine into a cluster can call it. See [API usage](/docs/api-usage) for details.

The web UI also exposes a **Cloud-init…** button on each cluster's overview page for one-click generation (available to any user with write access to the cluster).
