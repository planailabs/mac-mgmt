# mac-mgmt — design

Fleet management for AI infrastructure: one central server owns the desired state of
many machines ("instances"), a self-updating daemon on each machine reconciles towards
it, and a relay gives operators NAT-traversing access to those machines.

Where this document stops: per-crate internals live next to the code —
[`daemon/DESIGN.md`](daemon/DESIGN.md) and
[`mac-mgmt-services/DESIGN.md`](mac-mgmt-services/DESIGN.md). Build and test rules live
in [`AGENTS.md`](AGENTS.md); this file is about the shape of the system and why.

## 1. The problem and the shape of the answer

A fleet of GPU boxes behind unknown networks runs agentic workloads (OpenClaw, Ollama,
LiteLLM, Hermes, …). Operators need to install, configure, watch and repair them
without inbound access to those boxes, and without a person on site.

Four processes answer that, and the trust boundaries between them explain most of the
code:

| Process | Runs where | Owns |
|---|---|---|
| **server** (`mac-mgmt-server`) | one host | The database: clusters, configs, tokens, skills/MCP catalogs, rollouts, healer sessions. The only writer of desired state. |
| **daemon** (`mac-mgmt`) | every instance | Local reality: services, nix, probes, self-update. The only thing that touches a managed machine. |
| **relay** (`mac-mgmt-relay`) | one host (public) | Reachability only: p2p circuit relay, per-daemon SSH ports, subdomain HTTP proxy. Deliberately holds no fleet state. |
| **supervisor** (`mac-mgmt-services`) | every instance | Process lifetime of managed services, across daemon upgrades. |

Two derived systems ride on top: the **healer** (an LLM agent that diagnoses and repairs
instances through the relay) and the **runner / mmr-causality** (test fleets on Incus).

## 2. Control flow

```
                        ┌───────────────────────── server ─────────────────────────┐
                        │ Postgres (sqlx)  ·  Rocket /api  ·  Axum web UI + /api/v1 │
                        └───┬─────────────┬──────────────────────┬─────────────────┘
      config / heartbeat /  │             │ SSE push             │ relay auth
      probes (sync token)   │             │ (SyncConfig, …)      │ (cert-auth)
                            │             ▼                      │
                   ┌────────┴────────┐                    ┌──────┴───────┐
                   │     daemon      │◀── libp2p ────────▶│    relay     │
                   │ reconcile+probe ├── file/shell/tcp ─▶│ p2p + proxy  │
                   └────────┬────────┘    tunnels          └──────┬───────┘
                            │ unix socket / named pipe             │
                   ┌────────▼────────┐                     browser / healer / mmrc
                   │   supervisor    │
                   └─────────────────┘  → openclaw, ollama, litellm, …
```

- **Pull, not push, for desired state.** The daemon polls `GET /api/config` on the
  update interval and *also* reacts to an SSE `SyncConfig` event; the poll exists so a
  lost push is self-healing. Remote JSON config is migrated
  (`common/src/config_migrate.rs`) before deserialization, and **local TOML wins over
  remote JSON** — the machine's own file is the last word, so a server outage or a bad
  remote config degrades instead of bricking a box.
- **Push, not pull, for state.** Heartbeats, samples and probe reports go up; the server
  keeps them in `daemon_heartbeats` / `assessments` / `assessment_probes`.
- **Everything the daemon does is on a tick.** One `select!` in `daemon::run()` drives
  update (1h), health + heartbeat (1m), config poll (1h), inventory (6h), liveness
  (60s ±10s) and functional probes (30m ±120s), backup (6h) and nix GC (24h). Intervals
  come from `[daemon] update_interval` / `health_interval` and are re-armed live on
  config reload, so changing a cadence needs no restart.

### Three-tier assessment

The fleet view is built from three cadences with different costs, which is what keeps
the server's tables from growing with instance count × time:

| Tier | Cadence | Cost | What it answers |
|---|---|---|---|
| Sample | every heartbeat (1m) | cheap sysinfo | CPU/mem/disk/net/GPU right now |
| Liveness probe | 60s ±10s | pid/port checks | is the process up |
| Inventory | 6h | `nix`, disk, security audit | what is this machine |
| Functional probe | 30m ±120s | a real LLM round-trip | does it actually work |

The jitter is not cosmetic: without it every instance in a cluster would hit the same
backend in the same second.

## 3. Crate layout

`Cargo.toml` has **25 workspace members** and an explicit `exclude` list; the exclusions
are load-bearing, not tidiness:

- `memvault` — an independent workspace **submodule**. Cargo cannot nest workspaces, so
  crates here depend on its crates (`memvault-*`, `memctl`, `memvault-web`) by path.
- `common` — also a **submodule**, shared with sibling projects.
- `design` — a symlink to the nested `memvault/plan-ai-design` submodule. It exists for
  Cargo lockfile *path identity*: `plan-ai-design` must resolve to one textual path
  everywhere, or Cargo sees two packages. `scripts/canonicalize-design-path.py`,
  `build-memvault.sh`'s manifest rewrite and CI's `MEMVAULT_KEEP_CANONICAL_MANIFEST=1`
  are all the same workaround.
- `mac-mgmt-overview` and `config-ui` — each has its own `[workspace]` root and is
  depended on by path; `mac-mgmt-overview` behind the daemon's `usb-ui` feature so
  headless builds never pull in the wry/webkitgtk desktop stack.
- `common/plan-ai-actions` — excluded because its `plan-ai-design` path dep only
  resolves in the *web-agency* layout, and web-agency is one of its only consumers.

```
common/            shared wire types, ClusterConfig/DaemonConfig, config_migrate,
                   custom_service, framing, otel/sentry/metrics  (+ plan-ai-* sub-crates)
mac-mgmt-agent/    the reusable agent control plane (see below)
daemon/            the daemon binary — CLI, dashboard, self-update, USB stack
mac-mgmt-services/ process supervisor (separate binary, stable protocol)
server/            management server: Rocket /api + Dioxus web UI + Axum /api/v1 + /mcp
relay/             p2p circuit relay, SSH bridge, subdomain HTTP proxy, web terminal
relay-ssh/         CLI SSH client into relay-managed machines
runner/            Incus test-fleet orchestrator daemon
mmr-causality/     mmrc CLI + mmrcd orchestrator for prod/antithesis chaos
mac-mgmt-healer/   the LLM repair agent (sessions, tools, skills)
mac-mgmt-executor/ MCP server handing out ephemeral Incus containers
sim-tests/         deterministic simulation harness for the daemon
antithesis-workloads/  eventual-consistency workloads over HTTP
mac-mgmt-ws/       WebSocket utilities (currently unused by any crate)
plan-ai-cleaner/   PII-redaction MCP server        plan-ai-cloud/  cloud MCP server
plan-ai-memvault-share-agent/  cross-cluster share review MCP server
nix-driver-sync/   sync /run/opengl-driver into containers   trainer/  fine-tuning
```

### The `mac-mgmt-agent` extraction

`daemon/src/lib.rs` is **mostly a re-export facade**. Assessment, heartbeat, the service
supervisor and catalog, the relay/tunnel stack, config fetch/merge and embedded memvault
live in `mac-mgmt-agent` (~26k lines) and are re-exported under their old `crate::` paths
so daemon code keeps compiling.

The reason is a second consumer: plan-ai-usb's `usbd` needs the machinery but not the
daemon's CLI, dashboard, MCP and cloud surface. That is also why `mac-mgmt-agent` (like
`mac-mgmt-services`) uses **direct version deps rather than `workspace = true`** — it must
build standalone as a path/git dependency outside this workspace.

Practical consequence: a symbol used in the daemon usually originates in
`mac-mgmt-agent/src/`, and `daemon/src/lib.rs`'s `#[cfg(feature = …)]` lines are the map
of what belongs to whom. The daemon's features (`services`, `relay`, `memvault`,
`self-update`, `usb`, `usb-ui`, `healer`) mirror and forward to the agent's.

## 4. Server

Both a Rocket API and a Dioxus fullstack app in one process, on two ports:
**Rocket `/api` on 7378** (Swagger at `/api/swagger-ui`), **Axum web UI on 7377**, which
also carries the SSE endpoints, the `/api/v1` REST surface and the MCP mounts.

Three API surfaces exist because they grew for different consumers:

| Surface | Auth | For |
|---|---|---|
| `/api` (Rocket, `server/src/api/`) | bearer token, kind-checked request guards | daemons and the relay — the original surface |
| `/api/v1` + `/mcp` (`server/src/api_mcp/`) | `api`/`admin` token or MCP session | external automation and LLMs, one registry served three ways |
| Dioxus server functions (`web/`) | OIDC cookie session | the web UI itself |

The `api_mcp` layer is the interesting one: `build_registry()` declares 18 resources once,
and `#[api_mcp_dioxus_server]` emits the REST route, the MCP tool and the Dioxus
`#[server]` wrapper from the same handler — so the UI, the REST API and MCP cannot drift
apart. It is also why the web UI's types are shared rather than duplicated.

**Auth** is by token *kind*, stored hashed in `tokens`:
`sync` (one cluster, daemon polling) · `setting` (cluster or org, catalog/config) ·
`admin` (everything) · `api` (external, org-scoped, read/write via `scopes.role`) ·
`federation` (skill center ↔ server) · `proxy` (6h TTL, browser tunnel access).
The relay authenticates with **client certificates** (`/api/cert-auth`, x509), not a
bearer token. Web users come from OIDC with org roles `read`/`write`/`admin`, and cluster
access derives from `organization_clusters`.

**Federation** is how several management servers share a catalog: `GET /federation/catalog`
is `FederationOrPublic` (anonymous sees only non-hidden entries), `POST /federation/resolve-*`
pulls a skill/MCP-server definition transitively, and `GET /federation/events` is an SSE
stream. Every catalog mutation path must call `crate::api::push::notify_federation_global()`
or subscribers never learn about it — hence the rule in `AGENTS.md`.

**Migrations are append-only**, and there are two chains: `plan-ai-chat`'s (unified chat
tables) runs *before* `sqlx::migrate!()`, because server migration `067` folds the old
healer tables into the chat tables. Editing an applied migration is a no-op on deployed
instances plus a checksum mismatch — always add the next number.

`ServerMode` (`monolith` | `mgmt` | `skill-center` | `skill-importer`) gates which route
modules mount at runtime, so the same binary is a full server, a management-only server,
or a catalog-only skill center.

## 5. Daemon

The daemon's job is to make the machine match the config and to tell the server what it
found. Its surface is wider than that suggests:

- **Service management.** A `ManagedService` trait
  (`mac-mgmt-agent/src/managed_service.rs`) with ~25 impls — openclaw, ollama, lms,
  litellm, hermes (4 variants), opencode, apprise, mcporter, restic, nvidia/rocm-smi,
  memvault, relay, ai-proxy, swarm, unsloth, custom. Each declares install, spawn,
  health, tunnels, file tunnels, shell commands and probes; `connectors::build_services`
  assembles the active set from config. `[[custom-service]]`
  (`common/src/custom_service.rs`) exposes the same surface declaratively, so a service
  needs no Rust to be supervised.
- **The supervisor is a separate process.** Services are spawned by `mac-mgmt-services`,
  reached over a unix socket (a named pipe on Windows; `MAC_MGMT_RUNTIME_DIR` relocates
  it off a FAT32 home dir where binding a socket is EPERM). That separation is what lets
  the daemon self-update and re-exec without dropping children. Its protocol is
  **two-way backwards compatible by rule** (see `AGENTS.md`): daemon and supervisor
  upgrade independently, so new fields go behind `#[serde(default)]`, replaced fields
  stay on the wire during transition, and every shim carries a
  `// compat: added …, removable after …` comment.
- **Self-update.** Version check → `nix-store --realise` → symlink swap → restart, gated
  by an upgrade window. An interval is chosen to avoid all instances restarting at once.
  There is **no staged rollout and no automatic rollback** in the binary updater; the
  staged, health-gated machinery is the server's *rollout* feature, which drives which
  version instances are told to run (see `runner/README.md` and `server/docs/*/rollouts.md`).
  `canary.rs` is about cloud-provider canary endpoints, not binary rollout.
- **Relay side.** The daemon dials out over libp2p (circuit relay through the relay's
  `p2p_port`), registers file/shell/TCP tunnels, and gets a reserved SSH port with a
  30-day TTL so a reconnect reclaims the same port. No inbound firewall opening needed.
- **USB / sovereign build** (`usb`, `usb-ui`): a relocatable offline stack on a flash
  stick — portable nix, store image, embedded memvault web UI, self-contained tarball
  updater. It is why `main.rs` dispatches to `usb::main` *before* the tokio runtime
  starts: the desktop event loop must own the main thread.

## 6. Relay

Holds no fleet state, which is deliberate — losing the relay costs reachability, not
correctness. It runs a libp2p swarm (circuit relay + control protocol), per-daemon SSH
TCP bridges (`ssh_port_min..max`, default 30000–40000), an HTTP proxy that routes
`{instance_prefix}-{tunnel_name}.{proxy_hostname}` to the right tunnel, a web terminal
bridging axum WS ↔ libp2p substreams, and mTLS with per-SNI client-cert behaviour — plus
an HTTP/2 **connection-coalescing guard** returning 421 when the Host's class disagrees
with the TLS class the connection was accepted with, so a proxy-subdomain request can
never ride a main-domain connection and skip cert checks.

Tunnel traffic uses the shared framing in `common/src/framing.rs` (tagged JSON/binary/
end frames, 16 MiB max). Shell tunnels distinguish **virtual** commands — handled
in-process with no spawned process (`service-restart`, `restart-daemon`, `sync-state`,
`memctl`) — from real commands with per-command timeouts.

## 7. Healer

An LLM agent that repairs instances, running inside the server process. It reaches a
machine only through the relay, with a scoped proxy token minted per session
(`files:read|write`, `shell:exec`, `logs:read`) — the agent has no ambient authority.

It tries local Ollama first (zero marginal cost) and falls back to cloud providers, with
a **token budget** (default 200k) that auto-pauses a session rather than letting a loop
burn money; `extend-budget` raises it to 1M. Sessions live in the unified chat tables
with `session_type='healer'`, so interrupted sessions resume on server restart.

States: `Created → Initializing → Diagnosing → Remediating → Verifying → Done`, with
`NeedsHumanAttention`, `AwaitingApproval`, `AwaitingRetry` (auto-resume), `Paused` (budget
or manual) and `Cancelled` as exits. Its output for humans is **staff pings** —
categorized, resolvable notifications — and its step-by-step knowledge is embedded
remediation **skills** (`mac-mgmt-healer/skills/`).

Sessions are spawned three ways: the API, the auto-trigger loop on unhealthy instances,
and the `/mcp/healer` MCP server (which is how an external agent drives repair).

## 8. Testing

Five layers, cheapest first — pick the lowest one that can catch the bug:

1. **Unit / in-crate tests** — `cargo test -p <crate>`. Includes the config-migration
   idempotence tests and, in the server, the api-mcp registry tests that assert the
   OpenAPI document parses and the router mounts without route collisions.
2. **`sim-tests/`** — a deterministic harness: mock server, mock `ManagedService`,
   seeded scenarios, invariants, a sampler/minimizer and a timeline. Real daemon code,
   no VM, seconds to minutes. This is what `/chaos-test` drives.
3. **Nix VM checks** (`tests/*.nix`, `nix flake check`) — `relay-integration`,
   `metrics-federation`, `sse-push`, `sse-daemon`, `usb`. Real NixOS VMs, minutes, and
   they must run serially (`--max-jobs 1`) or one VM starves another until the NixOS
   backdoor times out.
4. **`antithesis-workloads`** — eventual-consistency workloads run against a live
   cluster by `mmrc emulator` or on the Antithesis platform.
5. **CI** (`.gitlab-ci.yml`) — build → `cargo test --workspace --locked
   --exclude mac-mgmt-executor` → deploy stage running `nix flake check`, `xzar.sh`,
   `docker-push.sh`, `usb-release.sh` on `trunk` only.

`mac-mgmt-executor` is excluded from the workspace test run because its tests spawn the
binary and speak MCP over stdio to a **real `incus`** — they need infrastructure a CI
runner does not have.

## 9. Invariants and the rules that protect them

These are the constraints the code keeps re-stating; breaking one is how this system
fails interestingly.

| Invariant | Protected by |
|---|---|
| A machine keeps working when the server is unreachable | local TOML overrides remote JSON; config fetch failure falls back to local |
| An image rebuild never bricks a box | upgrade window + nix store-path swap; new binary verified before symlink swap |
| Daemon upgrades never kill running services | services live under a separate supervisor process, reached over a socket |
| An old and a new peer can always talk | `mac-mgmt-services` protocol compat rule + dated shim comments |
| An applied migration is never re-interpreted | append-only `server/migrations/` |
| Remote JSON configs on disk always deserialize | every config change ships a migration in `common/src/config_migrate.rs` + a SQL migration |
| The agent can only do what the session allows | per-session scoped proxy token, no ambient relay credentials |
| A cloud LLM loop cannot burn budget forever | per-session token budget with auto-pause |
| A request on a proxy subdomain never skips cert checks | HTTP/2 coalescing guard (421) in `relay/src/main.rs` |
| The server survives a slow daemon | every external command in the event loop goes through `cmd::output_with_timeout()` |
| `/api`, `/api/v1`, MCP and the UI agree | one `api_mcp` registry generates all three; UI types shared with the server |
| Every federated peer learns about catalog changes | `notify_federation_global()` on every mutation path |
| Docs stay usable in both shipped languages | `server/docs/{en,de}` with identical slugs, updated in the same commit |

## 10. Known rough edges

- `server/src/api/routes.rs` is ~240 KB — the largest file and the main reason the
  server is hard to navigate; new work should go through `api_mcp/endpoints/`.
- `mac-mgmt-agent/src/service_mgmt.rs` (~104 KB) and the daemon's `daemon.rs` (~100 KB)
  carry the same problem on the daemon side.
- `mac-mgmt-ws/` is a workspace member with no dependents; the relay and daemon use
  `tokio-tungstenite`/axum WS directly.
- `README.md` lags the code in several places (see "Documentation" below).
- `web-agency/`, referenced by `AGENTS.md` for OpenAPI client regeneration, is a sibling
  project and **not present in this checkout** — that section is unactionable here.

## 11. Documentation map

- `README.md` — feature overview and quick start (partly stale; this file is canonical
  for architecture).
- `AGENTS.md` — build/test rules, feature-combination matrices, and the "always do X"
  constraints (config migrations, protocol compat, bilingual docs, chaos testing).
- `DESIGN.md` (this file) — system architecture.
- `daemon/DESIGN.md`, `mac-mgmt-services/DESIGN.md` — crate internals.
- `runner/README.md`, `mmr-causality/README.md` — test-fleet orchestration.
- `server/docs/en/*.md` + `server/docs/de/*.md` — end-user docs (18 each, identical
  slugs); `server/glossary/*.md` — terminology.
