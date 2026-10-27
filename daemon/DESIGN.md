# Daemon (`daemon/`, package `mac-mgmt`) — architecture

Single Cargo package, two targets: bin `mac-mgmt` (`src/main.rs`) and lib `mac_mgmt_daemon`
(`src/lib.rs`). The same binary is a WASM client entry when built with `--features web`
(`main.rs:5-7`).

## 0. Where the code actually lives (read this first)

`daemon/src/lib.rs` is mostly a **re-export facade**. The control plane (assessment, heartbeat,
service supervisor, relay/p2p, memvault, config, connectors) was extracted into a sibling crate
**`mac-mgmt-agent`** (~25k lines) and is re-exported under the old `crate::` paths so daemon code and
the CLI/dashboard surface compile unchanged (`daemon/src/lib.rs:8-70`). `mac-mgmt-agent/Cargo.toml`'s
header states why: consumers such as plan-ai-usb's `usbd` need the machinery without the daemon's
CLI/dashboard/MCP/cloud surface. That crate uses *direct* version deps (not `workspace = true`) so
it builds standalone as a path/git dependency outside this workspace.

Consequence: a symbol used in the daemon usually originates in `mac-mgmt-agent/src/`. Below,
**⟨agent⟩** marks re-exported modules.

## 1. Module tree

### Owned by the daemon (`daemon/src/`)

| Module | Lines | What it is |
|---|---|---|
| `main.rs` | 42 | argv0 dispatch: basename `systemctl` → `systemctl_main()`; with `usb` feature and arg1 absent/`usb` → `usb::main(args)` **before** the tokio runtime (the desktop event loop must own the main thread, a hard requirement on macOS); else `daemon_main::main()`. WASM: `memvault_web::launch_client()`. |
| `lib.rs` | 79 | Re-export facade + `GIT_SHA` (from `build.rs`). |
| `daemon_main.rs` | 622 | CLI (clap), tracing/sentry init, subcommand dispatch (status/logs/usb/self-update/systemctl), then `daemon::run()`. |
| `daemon.rs` | **2536** | The core. `struct Daemon` (`:26`), handlers (`handle_update`, `handle_health_tick`, `send_heartbeat`, `handle_config_reload`, `handle_push_cmd`, …), `run()` (`:925`) with the main `select!` loop (`:1520`), `run_sim()`/`run_sim_with_services()` variants, and free fns (`fetch_target_version`, `fetch_nix_caches`, `fetch_nixpkgs_pin`, `upgrade_nix`, `run_nix_gc`, `disk_threshold_exceeded`). |
| `self_update.rs` | 558 | Version check, nix store-path swap, symlink install, restart signalling. §5. |
| `systemctl.rs` | 545 | `systemctl` argv0 shim + `mac-mgmt systemctl` subcommand mode. Verbs `start/stop/restart/status/is-active/kill`, `list-units`, `cat`; unrecognised verbs and non-managed units pass through to real systemctl in argv0 mode, error otherwise (`:1-30`). Maps managed units onto the supervisor. |
| `usb.rs` | 591 | Sovereign-AI USB stack entry (`pub fn main(args) -> !`, `:147`), `NetworkMode` (`:36`). |
| `usb/` | 7 files | `control.rs` (321), `runtime.rs` (200), `store_image.rs` (296), `nix_portable.rs` (295), `nix_darwin.rs` (231), `nixpkgs.rs` (99), `prefetch.rs` (198): a portable nix runtime materialised onto the stick; `store_image` = store image, `nix_darwin` = macOS path. |
| `usb_update.rs` | 300 | Self-update for the USB/self-contained build (tarball + extraction; `tar`/`flate2`/`self-replace` under `usb`). |
| `skills.rs` | 285 | Syncs agent skills from the server (`builtin://` prefix) and chmod-protects the skills dir; unix-gated (`:1-20`). |
| `packages.rs` | 239 | Unified nix package sync aggregating MCP servers, skills and manual cluster additions via `GET /api/packages`; doc note says it *replaces* the old MCP-specific sync in `mcp_servers.rs` (`:1-4`). A global `nix_state_lock()` mutex serialises the read-modify-write. |
| `os_mgmt.rs` | 283 | Embedded OS config files (`os_mgmt/` holds an ubuntu apt unattended-upgrades snippet) applied as root via sudo. |
| `healer_bridge.rs` | 356 | Bridges daemon → `mac-mgmt-healer`: `LocalInstanceAccess`, `LocalInstanceDataSource`, `LocalSessionFactory` implement the healer's abstract traits so it can act on this daemon's file tunnels, shell commands, log buffer and assessment data with no relay or DB (`:1-6`). `healer` feature. |
| `metrics_server.rs` | 137 | Rocket server on `[::1]:<metrics.port>` (default **9396**, `config::read_metrics_port`): `GET /metrics` (Prometheus), `GET /status`. `SyncTrigger` channel. |
| `logs.rs` | 94 | `mac-mgmt logs` CLI: tails the ring buffer over HTTP (`GET /logs?n=&service=`) on the metrics port. |
| `crash.rs` | 46 | Sentry init + panic hook that reports and *attempts a self-update before aborting*. Hardcoded DSN with a `// TODO` (`:1`). |
| `config_watch.rs` | 50 | `notify` watcher on the config file's parent dir (non-recursive), matching `Modify`/`Create`; `recv_debounced()` drains further events for 1s. |
| `status.rs` | 112 | `mac-mgmt status` CLI: GETs `/status`, formats uptime + services. |
| `dashboard.rs` | 277 | Local HTML dashboard (Mustache via `ramhorns`, template `dashboard.html`) at `GET /dashboard`; no JS, mirrors the server fleet-detail design language. |
| `ai_proxy/` | 1315 | OpenAI-compatible proxy fronting local *and cluster* backends. `mod.rs` (198: `PeerProxyRequest` for libp2p forwarding, SHA2-256 multihash `0x12`), `server.rs` (392: Rocket `POST /v1/chat/completions`), `backend.rs` (289: `ResolvedBackend::{Local,Peer}`), `auth.rs` (79: `AuthedKey` bearer extractor, hashes the key, enforces token budget), `usage.rs` (181: `UsageEvent` tokens+latency), `types.rs` (158). |
| `mcp_servers.rs` | 60 | Fetches `GET /api/mcp-servers` and installs MCP server configs (installation moved to `packages.rs`). |
| `scripts.rs` | 43 | `rust-embed` of `daemon/scripts/`; `run(name)` writes to a tempfile, chmod 0755, executes. |

### Re-exported from `mac-mgmt-agent` ⟨agent⟩

`assessment`, `canary`, `cmd`, `config`, `config_providers`, `connectors`, `embed_write`, `events`,
`file_tunnels`, `heartbeat`, `host_keys`, `local_client`, `log_buffer`, `log_layer`,
`managed_service`, `memvault`, `metrics`, `nix`, `notify`, `p2p`, `platform`, `remote_ssh`,
`secrets_cache`, `server_push`, `service`, `service_mgmt`, `services`, `shell_tunnels`,
`unmanaged`, `validator` (`daemon/src/lib.rs:11-70`).

## 2. Main loops

One `tokio::select!` in `daemon::run()` (`daemon.rs:1520`); tick sources built at
`daemon.rs:1175-1201`.

| Tick | Interval | Source | What one iteration does |
|---|---|---|---|
| `update_tick` | `daemon.update_interval`, default **1h** (`common/src/lib.rs:944`) | `:1175` | `handle_update()` (`:103`): fetch `/api/update` target + nixpkgs pin + nix caches (awaited); if inside the upgrade window → `self_update::check_and_apply()` under a 120s timeout in `spawn_blocking`, then tell the supervisor `send_update_self()` (skipped on the first tick, `first_update` comment `:52-56`); `spawn_blocking(upgrade_nix)`; always `spawn_sync_skills_and_mcp()` and `refresh_schemas()`; `svc_mgr.check_upgrades()`. Then `check_binary_changed()` → if the install symlink moved, set `restart_exec_bin`, `handle_shutdown("self-update")`, break (`:1527-1535`); `relay_mgr.sync_ssh_keys()`. |
| `health_tick` | `daemon.health_interval`, default **1m** (`common/src/lib.rs:948`) | `:1176` | `svc_mgr.run_connectors_tick()` via `block_in_place` *before* health, so patched config/env is visible when new services register (`:1540-1546`); `handle_health_tick()` (30s timeout around `svc_mgr.health_tick`, then `maybe_trigger_healer`); relay tcp/file/shell tunnel def refresh; `refresh_assessment_sample()`; disk-pressure nix GC spawn (`nix_gc.disk_threshold_percent`); `send_heartbeat()`; `memvault.tick()`. |
| `heartbeat_tick` | same as `health_interval` (**1m**) | `:1177` | Lighter: `refresh_assessment_sample()` + `send_heartbeat()`. |
| `config_poll_tick` | `update_interval` (**1h**) | `:1180` | `handle_config_reload()`. Belt-and-braces: catches server-side config changes when the SSE `SyncConfig` push was missed or unavailable (`:1178-1179`). |
| `assessment_inventory_tick` | **6h** (`assessment::DEFAULT_INVENTORY_INTERVAL`) | `:1181` | `send_assessment_inventory()`. |
| `liveness_probe_tick` | **60s** ±10s jitter | `:1182` | `run_assessment_probes(ProbeKind::Liveness)`. |
| `functional_probe_tick` | **30min** ±120s jitter | `:1186` | `run_assessment_probes(ProbeKind::Functional)`. |
| `backup_tick` | `backup.interval` (default 6h); effectively never when disabled | `:1190-1197` | `handle_backup()` (restic), 3600s timeout. |
| `nix_gc_tick` | `nix_gc.interval`, default **24h** | `:1199` | `run_nix_gc("scheduled")`, guarded by an `AtomicBool` so only one GC runs at a time. |

Other arms of the same `select!`:

- `sigterm`/`sigint` → `handle_shutdown`, break (`:1522`).
- `svc_mgr.install_rx` (`:1585`) → incremental background installs; `None` means `finish_installs()`.
- `config_watch::recv_debounced` (`:1644`) → `handle_config_reload`, which **re-arms `update_tick`
  and `health_tick` in place** (and `config_poll_tick`) when the interval strings changed (`:172-195`).
- SSE push (`push_rx`, `:1652`): `SyncConfig` is special-cased in the loop because it needs the
  interval refs (`:1656`); the rest go through `handle_push_cmd` (`:773`). Ends with a
  `take_restart_exec()` check.
- `sync_rx` local sync (`:1674`); `relay_mgr.recv_cmd` (`:1680`); relay heartbeat signal (`:1690`);
  p2p event (`:1700`) — `P2pEvent::RelayProxyUrlAcquired` forces an immediate heartbeat so the server
  learns the proxy URL without waiting for a tick.

Shutdown: `svc_mgr.shutdown()`, `relay_mgr.cleanup()`, then if a self-update completed, `exec` the new
binary **through argv[0]** — argv[0] is the symlink, already repointed, so exec picks up the new
version instead of the resolved store path (`:1730-1744`).

`handle_config_reload` (`:149`) calls `crate::config::reload()` under a 10s timeout, applies
`set_log_level`, pushes the new config into `assessor.update_config()`, and hot-swaps intervals.

### Heartbeat and SSE

- **Heartbeat** (`send_heartbeat` `:401`; body built by `do_send_heartbeat`): service statuses; tcp
  tunnels (`name`+`port`); file tunnels (name/service/path/writable/description/kind, plus `include`
  for directories); shell tunnels (name/service/description/requires_arg/arg label+placeholder); the
  cached dynamic **sample**; latest **probe** summaries; per-service samples; and **failure signals**
  (resource pressure from `signals::evaluate_sample_signals` plus supervisor drift signals). POSTs are
  serialised by `heartbeat_lock: Arc<Mutex<()>>` so a faster later send can't overtake an earlier one
  and leave the server with a stale snapshot (`:57-59`, `:519-523`). On the *first* successful
  heartbeat it sends the initial inventory once, gated by `initial_assessment_pending` because
  migration 031's FK needs the parent row first (`:548-559`).
- **SSE** ⟨agent⟩ `server_push.rs`: `GET {server}/api/events` with bearer auth, `PushCommand` parsed
  from `data:` lines, reconnecting with exponential backoff 1s→60s (`MIN_BACKOFF`/`MAX_BACKOFF`
  `:8-9`) and a 90s read timeout (the server keepalives every 30s). `PushEvent::Targeted{instance_id,
  event}` is unwrapped and dropped here if it isn't ours; `Ping` is filtered here too — hence the
  `unreachable!()` arms in `handle_push_cmd` (`:776-779`). Commands: `SyncConfig`, `SyncSkills`,
  `SyncMcpServers`, `SyncPackages`, `SyncSshKeys`, `SelfUpdate`, `SyncNixpkgs`,
  `RequestAssessment`, `Ping`, `Targeted`.

## 3. Three-tier assessment ⟨agent⟩ (`mac-mgmt-agent/src/assessment/`)

Header `assessment/mod.rs:1-10` documents the design directly.

| Tier | Constant | Cadence | Collected by | Sent to |
|---|---|---|---|---|
| **Sample** | — (piggybacked) | every heartbeat (~1m) | `sample::collect()` — CPU/mem/disk/net, <50ms | inside the heartbeat POST |
| **Inventory** | `DEFAULT_INVENTORY_INTERVAL` = **6h** (`:37`) | startup, 6h, and `RequestAssessment` | `inventory::collect()` + `security::collect()` + `svc_mgr.collect_service_inventories()` / `collect_service_security()` | `POST /api/assessment` |
| **Probe: liveness** | `DEFAULT_LIVENESS_INTERVAL` = **60s**, jitter ±10s (`:33`) | 1m | probes with `kind() == Liveness` (pid/port) | `POST /api/assessment/probe` |
| **Probe: functional** | `DEFAULT_FUNCTIONAL_INTERVAL` = **30min**, jitter ±120s (`:35`) | 30m | probes with `kind() == Functional` (full LLM round-trip canaries) | `POST /api/assessment/probe` |

`pub struct Assessor` (`:41`) holds latest sample/probes/config/metrics snapshots plus
`last_probe_runs`, so probes with a custom `interval()` are skipped when not yet due (`:50-52`).
`jittered(base, secs)` (`:294`) is what the daemon ticks use. Entry points: `refresh_sample` `:92`,
`send_inventory` `:109`, `run_probes_filtered` `:170` (registry from `probes::registry(&cfg)`,
filtered by `ProbeKind`), `request` `:261` (push-triggered). Assessment bodies are **signed with the
SSH host key** (`build_signed_assessment`) before POSTing. Probe modules: `probes/{ollama, openclaw,
hermes, mcporter, custom, …}`; support modules `gpu.rs` (668), `security.rs` (307), `inventory.rs`,
`sample.rs`. Each tier also feeds Prometheus gauges via `attach_metrics`.

Healer auto-trigger (`daemon.rs:655`): with `healer.auto_trigger`, count consecutive health ticks
where any probe is unhealthy; default threshold **10** ticks (~10 min at a 1m interval), then start a
local healer session seeded with the latest sample JSON.

## 4. Managed services ⟨agent⟩

**`ManagedService` trait** — `mac-mgmt-agent/src/managed_service.rs:186`, ~30 methods, `Send + Sync`.
Not `#[async_trait]`: the async hook `check_health_async` returns a boxed future (`:226`).
Required: `name`, `ensure_installed`, `ensure_setup`, `spawn_spec`, `check_health`, `repair`,
`check_and_upgrade`. Defaulted: `service_mode` (an `InstallOnly` mode bypasses supervision),
`binary_name`, `preflight`, `post_start`, `persist_connector_env`, `is_busy`, `configure`,
`supports_hot_reload`, `needs_restart`, `register_metrics`, `collect_metrics`, `expose_tunnels`,
`tunnel_overrides`, `expose_files`, `expose_shell_commands`, `service_inventory`, `service_sample`,
`service_security`, `data_paths`. Tunnel descriptor is `TunnelDef{name, tcp_port, …}` (`:24`).

**Catalog** — `connectors::build_services(cfg)` (`connectors/mod.rs:83`) instantiates by per-provider
`enabled` flag:

- Always: `McPorter`, `Apprise`, `NvidiaSmi`, `RocmSmi`.
- Conditional: `OpenClaw`, `Opencode`, `Hermes` (+ `HermesDashboard`, `HermesWebui` sub-entries),
  `Ollama`, `Lms`, `Unsloth`, `Litellm` (when `litellm.enabled`), `Restic` (backup), integrated
  `AiProxyService`, integrated `MemvaultService`, and one `CustomService` per enabled
  `[[custom-service]]`.
- `ServiceMode::InstallOnly` services go to a separate `install_only` list
  (`service_mgmt.rs:323-330`).
- Swarm/relay integrated services are registered *before* P2P init so a failed swarm reports
  unhealthy rather than missing (`daemon.rs:1242-1260`).

**Connectors** — `connectors/mod.rs:218` `build_connectors`, the `relay_*`/`ollama_*`/`lms_*`/
`litellm_*`/`cloud_*`/`memvault_*` → `{hermes, openclaw, opencode}` matrix, plus `backup`. They patch
other services' config files and collect env vars; they run on the health tick *before* it.

**ServiceManager** — `service_mgmt.rs:181`. Per-service `ServiceState` with phases (`CrashBackoff`
`:62`, `Installing`, `InstallFailed`, `Healthy`, …); exponential crash backoff 5s/10s/… via
`crash_backoff_delay` (`:127`); `restart_pending`/`restart_at`; `drift_signals()`; `collect_statuses`
/ `collect_tunnels` / `collect_file_tunnels` / `collect_shell_tunnels` / `collect_service_samples`;
`check_upgrades()`; `health_tick(metrics, UpgradeGate)` (`:938`, phased: crash-backoff expiry via
`expire_crash_backoff`, then per-service `lifecycle_step` — restart pending, drift detection via the
supervisor's resolved store paths, upgrade apply — then health checks);
`send_update_self()`; `run_connectors_tick()`; `retry_failed_installs()`; `shutdown()`.
A service in `CrashBackoff` skips restarts (the supervisor already respawned it) but **not** a pending
upgrade: applying it is what can end the crash loop, and `check_upgrades()` skips services with
`upgrade_pending` set, so skipping the apply would starve them (ping f74b6db8).
`UpgradeGate::{Anytime, InWindow, Deferred}` is derived from the upgrade window (`daemon.rs:63-79`).

Supporting subsystems: `unmanaged/` — services the daemon didn't spawn but still manages
(`mod.rs`, `installer.rs`, `unit_generator.rs`, `manifest.rs`: generating/installing systemd units and
manifests). `service/` — per-platform backend selection: `systemd.rs` (331), `launchd.rs` (357),
`mod.rs` (127). `config_providers.rs` (207) — `ConfigStore` with change detection (`set` reports
whether the value changed, plus `any_changed`/`all_available`), shared between services and
connectors.

## 5. Self-update (`daemon/src/self_update.rs`, feature `self-update`)

Server-driven and nix-store-path based. **No staged/fractional rollout and no automatic rollback
exist in this crate** (unverified: no rollback code found).

1. **Target** — the server's `/api/update` returns a version plus the nix store path containing the
   binary for this system (resolved server-side from `daemon_versions`). Cached in a process-global
   `RwLock<Target>{version, store_path}` via `set_target` (`:33`); the daemon fetches it in
   `handle_update` via `fetch_target_version` (`daemon.rs:1784`).
2. **Gate** — `check_and_apply()` runs only inside the upgrade window (`daemon.rs:111`; the push path
   logs "outside upgrade window, deferring"). `daemon.upgrade_window` is `"HH:MM-HH:MM"`; the same
   value yields the service manager's `UpgradeGate`.
3. **Idempotence / downgrade guard** (`:143`) — refuses a downgrade when both versions are semver;
   non-semver ("rolling") has no ordering, so any move is allowed and the store-path comparison keeps
   it idempotent. The *primary* check compares the running binary's store path against the target
   store path — more reliable than the version string, since two builds of one version can live at
   different store paths. When the binary isn't in `/nix/store` (dev build, manual install) it falls
   back to an on-disk marker `${config_dir}/mac-mgmt/.mac-mgmt.store-path`; a changed store path
   under an unchanged version is deliberately re-applied.
4. **Apply** — `apply_store_path()` validates the path first (`is_valid_store_path` rejects
   `/etc/passwd`, `--eval`, `..` escapes and relative paths; covered by tests), then `self-replace`
   atomically swaps the binary (`dep:self-replace`, from the `self-update` and `usb` features).
5. **Symlink install** — `ensure_symlink()` (`:388`): if the binary is a plain file, `nix-store --add`
   it, replace the file with a symlink into the store, and register a GC root. Rationale in the doc
   comment: overwriting a *running* binary fails with "text file busy". Returns the stable symlink
   path for unit files. `install_bin_path()` (`:105`) prefers the original argv[0] when it is outside
   `/nix/store`, else `~/.local/bin/mac-mgmt`.
6. **Restart** — `RESTART_EXEC` is set, and the loop's `check_binary_changed()` (`:351`) compares
   `canonicalize(install symlink)` against `current_exe()`; if the symlink moved, the daemon shuts
   down cleanly and `exec`s argv[0] (through the symlink) rather than the resolved store path
   (`daemon.rs:1730-1744`).
7. **Supervisor** — after a successful apply the daemon calls `svc_mgr.send_update_self()` →
   `Request::UpdateSelf`; the supervisor re-execs and children survive reparenting seamlessly.
   `Response::NoChange` is returned when the supervisor binary is already current. Skipped on the
   first tick (`first_update`, `daemon.rs:52-56`), because the daemon itself hasn't restarted yet and
   asking the supervisor to re-exec now would be premature.
8. **Crash path** — `crash.rs`'s panic hook attempts a self-update before aborting (`:1-25`).
9. **Packaging** — `daemon/upload.sh` reads VERSION from `Cargo.toml`, `rsync`s `mac-mgmt.tar.gz` to
   `deploy@logos.plan.ai:<dest>/<version>/` and updates a `<dest>/mac-mgmt.version` pointer.
   `daemon/module.nix` exposes `services.mac-mgmt.{enable, serverUrl, version, system, settings,
   environmentFile, otlpHeadersFile}` and creates **two** units (daemon + services-supervisor)
   sharing `commonServiceConfig`/`commonUnitAttrs`: `Restart=always`, `RestartSec=5`, dedicated
   `mac-mgmt` user/group, `StateDirectory`, `AF_UNIX` allowed, a pinned `path` (nix + bashInteractive
   + coreutils + gitMinimal), `HOME=stateDir`, and a `requires` on `mac-mgmt-download.service` which
   fetches the versioned artifact before the daemon starts.
10. **Naming pitfall** — `mac-mgmt-agent/src/canary.rs` (re-exported as `crate::canary`) is about
    **cloud provider canary endpoints**, not staged binary rollout; its tests assert
    `cloud_canary(CloudProvider::from_name(…))` per provider (`canary.rs:90-138`). Don't conflate the
    two.

## 6. Relay / p2p from the daemon's side ⟨agent⟩

- **Swarm** — `p2p/mod.rs` (~2178). libp2p 0.56 with noise + yamux over tcp, plus dns, websocket,
  quic, relay (client), request-response, identify, mdns, ping (15s interval, `:386-388`),
  gossipsub and kad. Identity is derived from the **same ed25519 host key as SSH**
  (`p2p::identity::keypair_from_russh`); the comment at `daemon.rs:1262-1266` calls this "design A-1:
  node key = libp2p key" and stresses it is deliberately *not* a separate hash, so join/attestation
  peer matching lines up and memvault can share one swarm and one identity. Tunnel substream protocol
  is `/mac-mgmt/tunnel/1.0.0` (`:480`). `P2pEvent::RelayProxyUrlAcquired` (`:53`) is the event the
  daemon converts into an immediate heartbeat. Mdns discovery and Kademlia bootstrap run with a
  periodic bootstrap re-dial timer (`:594-602`).
- **Daemon wiring** — `RemoteSshState` is constructed with the server URL/token and
  `relay.remote_ssh_enabled` (`daemon.rs:1236`). The daemon pushes tunnel *definitions* into it on
  every health tick and install event: `update_relay_tunnel_defs` (tcp),
  `update_relay_file_tunnel_defs`, `update_relay_shell_tunnel_defs` (`daemon.rs:871-902`) — each
  short-circuits to empty lists when `relay.tunnels_enabled` is false. `update_relay_config` publishes
  `relay.proxy_hostname` plus the first 12 chars of the instance id into the service `config_store`
  (`:904`).
- **Tunnels** — `file_tunnels.rs` (528): `FileTunnelRegistry`, `handle_list`, `read_file`,
  `write_file`, `handle_read_session`/`handle_write_session`; the registry is refreshed from each
  service's `expose_files()`. `shell_tunnels.rs` (305): `ShellTunnelRegistry`, `VirtualOutput`,
  `register_virtual` handlers, `validate_args` + `build_command` (arg template), `handle_exec_session`.
  A distinct **"log tunnel"** mechanism was *not* found as a tunnel type; daemon logs reach the
  outside via `log_buffer.rs`/`log_layer.rs` and the SSH/relay paths (unverified).
- **SSH** — `remote_ssh/`: `mod.rs` (381), `ssh_server.rs` (265), `fifo_watcher.rs` (112), plus
  `pty.rs` and `ssh_keys.rs`; `russh` is the SSH library (a dep of both daemon and agent). PTY
  (`openpty`) and FIFO (`mkfifo`) are unix-only and `#[cfg(unix)]`-gated, so the Windows build never
  references them (`remote_ssh/mod.rs:1-12`). The separate top-level `relay-ssh/` crate is the
  client-side counterpart used to reach these sessions.
- **WS bridging** — daemon HTTP/WS traffic is bridged over p2p by the **relay** (`relay/src/ws_bridge.rs`):
  axum `WebSocket` ↔ libp2p substream, framed with tags from `mac_mgmt_common::framing`
  (`TAG_TEXT`/`TAG_BINARY`/close), length-prefixed, ping/pong handled locally. The daemon-side libp2p
  transport is `tokio-tungstenite` (agent dep) plus the swarm's websocket transport.

### Port reservation (lives in the relay, not the daemon)

`relay/src/daemon_registry.rs`: `DaemonConn` records the daemon's advertised tunnels, `ssh_port`
(set by `SshBridge`) and `peer_id`. Port allocation is a range scanner with persistence —
`allocate_port(instance_id)` first tries `claim_reservation`, else `take_free_port` (write lock held
across find+insert so concurrent callers can never collide), else expires reservations and retries.
`reserve_port(instance_id, port)` pins a port across reconnects for `RESERVATION_TTL_DAYS` = **30**
(`:6`) and persists to disk; `set_ssh_port`/`has_ssh_port`/`clear_ssh_port` do the same for SSH.
The default p2p port comes from `relay/src/config.rs:96` (`default_p2p_port`).

### The `relay/` crate, briefly

Separate binary `mac-mgmt-relay` (bin `mac-mgmt-relay`, `relay/src/main.rs`), modules: `api`,
`auth`, `config`, `daemon_registry`, `metrics_federation`, `mtls`, `p2p`, `proxy_handler`,
`ssh_bridge`, `ssh_identity`, `tunnel_io`, `web_ssh`, `ws_bridge`. It terminates TLS/mTLS, keeps the
live daemon registry, allocates the per-daemon public ports, bridges TCP tunnels and WS to libp2p
substreams, and serves web SSH. Local dev (`Procfile`): `server` via `dx serve`, `relay` via
`cargo watch -- cargo run`, logs to `/tmp/mac-mgmt-{server,relay}.log` (daemon logs to
`/tmp/mac-mgmt-daemon.log`).

## 7. Configuration

**Structs** live in `common/src/lib.rs` (this is the shared, server-and-daemon type crate):
`DaemonConfig` (`:2354`) is the **daemon's** view — it has `server: DaemonServerConfig`,
`opentelemetry` and `nix_gc` sections that `ClusterConfig` (`:2128`, the **server-side/cluster** view)
lacks. Shared sections appear in both. Other section structs: `AiProxyConfig` (`:1801`),
`BackupConfig` (`:1944`), `NixGcConfig` (`:2028`), `MemvaultConfig` (`:2072`), `RelayConfig`
(`:2289`), `CloudConfig` (`:1316`), plus `DaemonSettings`, `MetricsConfig`, `GlobalConfig`,
`OpenTelemetryConfig`, `NotificationsConfig`, `HealerClusterConfig`, and per-service configs
(`OpenClawConfig`, `OpencodeConfig`, `HermesConfig`, `OllamaConfig`, `OpenWebuiConfig`, `LmsConfig`,
`UnslothConfig`, `LitellmConfig`). Every field is `#[serde(default)]`; `ClusterConfig` fields also
carry `#[schemars(extend("x-category" = …))]` used by the config UI.

Top-level daemon sections: `daemon`, `notifications`, `global`, `openclaw`, `opencode`, `hermes`,
`ollama`, `openwebui`, `lms`, `unsloth`, `litellm`, `cloud` (a list), `metrics`, `opentelemetry`,
`server`, `relay`, `ai_proxy`, `healer`, `backup`, `nix_gc`, `memvault`, `custom-service`.
Defaults worth knowing: `update_interval` `"1h"`, `health_interval` `"1m"`, `log_level` `"info"`
(`common/src/lib.rs:944-954`), metrics port 9396. `DaemonSettings::validate()` checks the interval
strings parse as `humantime`, the log level is in `VALID_LOG_LEVELS`, and `upgrade_window` parses.

**Precedence: local TOML wins over remote JSON.** `mac-mgmt-agent/src/config.rs:232` `load()`:
read the local TOML at `config_path()` (`config_dir()/config.toml`), resolve `server.url`/`token`
(including `env:` refs from the `.env` file), fetch secrets from `/api/secrets` into a vault, then
`fetch_remote_config()` (`GET /api/config`, bearer; a 404 means "no remote config, skip"; the JSON is
cached to `config_dir()/.remote-config.json` for offline use). On fetch failure it falls back to the
cache. It then runs `config_migrate::migrate(&mut remote_json)` on the remote JSON **before**
merging, deep-merges the local JSON *over* the remote (`merge_json`, overlay wins), and migrates
**again** because the local overlay can reintroduce old-format fields (`config.rs:300-304`,
explicitly commented). Result is deserialised to `Config`; on failure it logs and **falls back to
the local config alone** (`:305-315`). Finally `resolve_config_secrets` substitutes `env:`/`secret:`
references, warning rather than failing. `merge_json` semantics (objects merge recursively, arrays
and mismatched types are replaced) are locked by tests, including one named
`local_overrides_remote_like_config_pipeline` (`config.rs:339-382`). Paths: `config_dir()` honours
`MAC_MGMT_CONFIG_DIR`, and `runtime_dir()` honours `MAC_MGMT_RUNTIME_DIR` and deliberately lives on a
FIFO/socket-capable filesystem because the USB build puts config on FAT32 where `mkfifo`/`bind` fail
with EPERM (`config.rs:20-45`).

**`common/src/config_migrate.rs`** (465 lines + tests) transforms **remote JSON** configs into the
current shape before deserialisation, so long-lived server configs and cached copies keep loading as
struct fields are renamed/restructured. `migrate()` composes ordered, **idempotent**
`migrate_NNN_description(&mut Value)` steps. Per `AGENTS.md`, any config struct change must add a step
(plus a `#[serde(alias = "old_name")]` on renames for local TOMLs, plus a SQL migration for
`cluster_configs` rows). The server applies the same migrations when reading configs from the DB and
before validating PUT/PATCH.

## 8. Cargo features (`daemon/Cargo.toml`)

`default = ["services", "relay", "memvault"]`.

| Feature | Gates |
|---|---|
| `services` | Service supervisor + catalog. Pulls `mac-mgmt-agent/services`, which itself requires `relay` (the supervisor wires p2p tunnel overrides into managed services). Enables modules `ai_proxy`, `healer_bridge`-adjacent service code, and re-exports `config_providers`, `connectors`, `managed_service`, `service_mgmt`, `services`, `unmanaged`. |
| `relay` | libp2p stack + tunnels: re-exports `file_tunnels`, `p2p`, `remote_ssh`, `shell_tunnels`; brings in `libp2p` directly. |
| `memvault` | Embedded memvault node (`mac-mgmt-agent/memvault`, `memctl/daemon`); implies `relay`. |
| `self-update` | `dep:self_update` + `dep:self-replace`; module `self_update`. |
| `healer` | `dep:mac-mgmt-healer`; implies `services` + `relay`; module `healer_bridge`. |
| `embed` | `memvault-web/embed` + `memctl/embed` — embedded UI assets. |
| `usb` | Sovereign-AI USB build: `services`, `memvault`, `embed`, `dep:self-replace`, `tar`, `flate2`, `schemars`, `axum`; modules `usb` + `usb_update`. |
| `usb-ui` | `usb` + `dep:mac-mgmt-overview` (native wry/webkitgtk desktop app). Kept separate so headless builds and CI never touch the GUI stack. |
| `sim` | `mac-mgmt-agent/sim` — deterministic-simulation hooks. |
| `web` | WASM client for the memvault web UI (`memvault-web/web`, `dioxus`, `getrandom_04/wasm_js`). |

`AGENTS.md` requires `cargo check -p mac-mgmt` to pass for: default; `--no-default-features`;
`--no-default-features --features self-update`; `--all-features`;
`--no-default-features --features services,relay,self-update`. Code behind one gate must not
reference items behind another without a matching `#[cfg]`.

## 9. Build & packaging

- `daemon/build.rs` (1568 B) — captures the git SHA at build time (`GIT_SHA` env or `git rev-parse
  HEAD`), exposed as `mac_mgmt_daemon::GIT_SHA` ("unknown" when neither is available).
- `daemon/module.nix` — the NixOS module; see §5.9 for the two units and options.
- `daemon/upload.sh` — release upload; see §5.9.
- `daemon/scripts/` — `setup.sh`, `tools.sh`, `ssh.sh`, `xrdp.sh`; embedded via `rust-embed` and run
  through `scripts.rs`.
- `daemon/os_mgmt/` — `ubuntu/etc/apt/apt.conf.d/50unattended-upgrades`, applied by `os_mgmt.rs`.
- `daemon/tests/integration/` — shell integration suite: `run.sh` (driver) + `lib.sh` (helpers),
  `setup-base-image.sh`, and per-feature tests `test_config.sh`, `test_self_update.sh`,
  `test_openclaw.sh`, `test_ollama.sh`, with fixtures `config.toml` and `config-with-extra.toml`.
- `daemon/Dioxus.toml` (86 B) — dx/WASM client config for the `web` feature build.
- `daemon/.gitignore` — build artifacts.
- Root-level `build-usb.sh`, `usb-release.sh`, `prebuild.sh` build/release the USB images around this
  binary (see the build-test-ci memory topic for detail).

## 10. Known-stale / caveats (per this survey)

- Protocol compat shims in `mac-mgmt-services/src/protocol.rs` are all past their stated removal dates
  as of 2026-10-10 — see `mac-mgmt-services/DESIGN.md` §3.
- `mac-mgmt-services/src/protocol.rs:81` documents a `Response::services_list` helper that does not
  exist.
- The hardcoded Sentry DSN in `crash.rs` still carries `// TODO: set your Sentry DSN` while a real DSN
  is in place — the comment is stale.
- `daemon/src/lib.rs` still describes its re-exports as being "under the old `crate::` paths for
  backwards compatibility"; the real implementation is in `mac-mgmt-agent`. Any doc describing
  `daemon/src/` as self-contained is out of date.
