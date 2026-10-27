# `mac-mgmt-services` — the service supervisor

A generic process supervisor with an RPC interface. The daemon tells it *what* to run; it spawns the
processes, restarts them when they crash, and streams logs and crash notifications back over the same
socket. The crate header states the guiding constraint: it is kept **deliberately small so the
long-running supervisor rarely needs to be re-executed when the daemon upgrades** (`src/lib.rs:1-7`).

For how the daemon consumes this (the `ManagedService` trait, the service catalog and
`ServiceManager`), see `daemon/DESIGN.md` §4. This document covers the supervisor itself and the wire
protocol.

## 1. Topology: a separate process over a local socket

- **The supervisor is its own process, not a library call.** The daemon connects to it as a client
  and issues `Register`/`Start`/`Stop`/… requests.
- **Socket path** — `default_socket_path()` (`src/lib.rs:22-38`): `$MAC_MGMT_RUNTIME_DIR/services.sock`
  when that env var is set, otherwise `$HOME/.config/mac-mgmt/services.sock`.
- **Why the runtime dir and not the config dir** — documented in the same function (`lib.rs:24-29`):
  on plan-ai-usb `HOME` is a FAT32 stick where binding a unix socket fails with `EPERM`, so the
  launcher pins `MAC_MGMT_RUNTIME_DIR` to a host-local directory. The agent side agrees on the path
  (`mac-mgmt-agent/src/config.rs::runtime_dir()`, which also documents the FAT32/EPERM constraint).
- **Transport** (`src/transport.rs`) — `interprocess` local sockets: a filesystem unix domain socket
  on unix, a namespaced named pipe on Windows. Both sides derive the endpoint from the same agreed
  `socket_path`, so they always meet at the same name; on unix a stale socket file from a previous
  run is removed before binding, and on Windows the path is only mapped to a pipe name (the
  relocation is inert there). `IpcStream` is the per-connection tokio `AsyncRead + AsyncWrite` stream.
- **Framing** — one `Message` per line (JSON via serde). Requests and responses carry a `type` tag
  (`rename_all = "snake_case"`); notifications arrive unsolicited on the same connection.
- **Cross-platform note** — `procutil.rs` (99 lines) holds the process utilities that differ per OS;
  `resolve_exe` uses `/proc/<pid>/exe` on Linux and is a no-op stub elsewhere (`server.rs:948-960`).

## 2. Wire protocol (`src/protocol.rs`)

```rust
enum Message { Request(Request), Response(Response), Notification(Notification) }  // tag = "kind"
```

### Requests (daemon → supervisor)

| Variant | Fields | Notes |
|---|---|---|
| `Register` | `name: String`, `spec: SpawnSpec` | Register or replace. If the spec matches the running one this is a **no-op**; otherwise the running process is killed and the new spec spawned. |
| `Unregister` | `name` | Stop and forget. |
| `List` | — | Currently registered service names. |
| `Stop` | `name` | Stop but keep the spec registered, so it can be restarted without re-supplying it. |
| `Start` | `name` | Start a previously stopped-but-registered service from its stored spec. |
| `Restart` | `name` | Stop then immediately restart. |
| `Kill` | `name`, `signal: i32` | Signal the service's main process. |
| `Shutdown` | — | Kill all children and exit the supervisor. |
| `UpdateSelf` | — | Re-exec the supervisor binary, used after the mac-mgmt binary was upgraded so the supervisor picks up the new code. |

### Responses (supervisor → daemon)

| Variant | Fields | Notes |
|---|---|---|
| `Ok` | — | |
| `Error` | `message` | |
| `NoChange` | — | `UpdateSelf` reply when the supervisor binary is already current, so no re-exec will happen. |
| `Services` | `statuses: Vec<ServiceStatus>`, `names: Vec<String>` | Both fields are populated from one list; see the compat section. |

### Notifications (unsolicited, supervisor → daemon)

- `Crashed { name, exit_code: Option<i32> }` — a managed child exited; the supervisor will respawn it.
- `Log { name, line, is_stderr }` — one line of stdout/stderr from a managed child.

### `ServiceStatus`

`name`, `pid: Option<u32>`, `exe: Option<String>` (resolved executable of the running child, e.g.
`/proc/<pid>/exe`), `resolved_program: Option<String>` (canonical path of `spec.program` resolved at
spawn via `which` + `canonicalize` — unlike `exe`, which points at the interpreter for scripts, this
always names the script/binary itself, which is what makes **store-path drift detection work for
shebang wrappers**), `spec: Option<SpawnSpec>`, `stopped: bool`.

### `SpawnSpec`

`program: String`, `args: Vec<String>` (`#[serde(default)]`), `env: HashMap<String,String>`
(`#[serde(default)]`).

### Building a compatible `Services` reply

`Response::services(statuses)` builds a `Services` response that is backward-compatible **on the
wire** by populating both `statuses` and the legacy `names` from the same list.

Stale reference: the `Response` doc comment tells readers to see `[Response::services_list]` "for a
helper that normalises the two", but **no `services_list` method exists** in the crate (grep finds
only this doc link, `protocol.rs:81`). Readers must normalise manually — prefer `statuses`, fall back
to `names`.

## 3. Two-way backwards compatibility (a hard project rule)

The supervisor and the daemon are upgraded **independently**: the supervisor keeps running across
daemon self-updates, and a new daemon commonly talks to an older supervisor (and vice-versa). Per the
project's `AGENTS.md`, any change to `protocol.rs` must stay compatible **in both directions**:

- New fields on existing variants go behind `#[serde(default)]` — readers that don't know them ignore
  them, and readers that expect them tolerate absence.
- When replacing a field, keep the old one on the wire during the transition and have the new side
  fall back to it when the new one is empty/missing (the `statuses`/`names` pair is the worked
  example).
- Never rename or remove an existing request/response variant; add new ones and keep handling the old
  shape.
- Mark every compat shim `// compat: added YYYY-MM-DD, removable after YYYY-MM-DD` (three months
  out), so a later cleanup pass can delete shims confidently instead of guessing what is still in the
  wild.
- When in doubt, test with one side on the old protocol and one on the new.

### Live shims in `protocol.rs`

| Location | Shim | Comment |
|---|---|---|
| `Request::Stop` / `Start` / `Restart` / `Kill` | new variants | `// compat: added 2026-05-20, old supervisors will ignore (unknown type)` |
| `ServiceStatus.exe`, `resolved_program` | `#[serde(default)]` | tolerate absence |
| `ServiceStatus.spec` | `#[serde(default)] Option<SpawnSpec>` | `// compat: added 2026-04-24, readers tolerate absence via serde(default)` |
| `ServiceStatus.stopped` | `#[serde(default)] bool` | `// compat: added 2026-05-20, readers tolerate absence via serde(default)` |
| `Response::NoChange` | new variant | `// compat: added 2026-07-06, old daemons error on the unknown variant` |
| `Response::Services.names` | legacy list alongside `statuses` | `// compat: added 2026-04-15, removable after 2026-07-15` |

Note: additions are *not* universally safe in the other direction — hence the two distinct comments.
New **request** variants are ignored by old supervisors because they don't recognise the `type`; a new
**response** variant, by contrast, makes an old daemon *error*, which is exactly why `NoChange` exists
as an explicit variant with its own compat note.

As of **2026-10-10** all three dated shims are past their stated removal dates — `names`
(removable after 2026-07-15), `NoChange` (2026-07-06) and `ServiceStatus.spec` (2026-04-24). Removing
any of them is a breaking protocol change against old peers, so confirm the fleet's actual version
floor before cleanup rather than trusting the dates alone.

## 4. Server loop (`src/server.rs`, 1228 lines)

- Entry: `pub async fn run(socket_path: &Path) -> Result<bool>` (`:61`) — binds the endpoint, accepts
  clients, and returns whether a re-exec was requested. Per-connection handling in `handle_client`
  (`:270`); writes go through `write_msg` (`:334`).
- State: `SupervisorState` (`:344`) holding `Entry`s (`:349`). Operations: `register` (`:415`),
  `adopt` (`:470`), `unregister` (`:513`), `stop_svc` (`:527`), `start_svc` (`:555`),
  `restart_svc` (`:591`), `kill_svc` (`:612`), `save_state_for_reexec` (`:634`), `abort_all_tasks`
  (`:678`), `shutdown_all` (`:689`).
- Child lifecycle: `run_service` (`:772`), `spawn_child` (`:819`), `wait_child` (`:850`),
  `stop_child` (`:877`), `monitor_adopted` (`:709`), `spawn_log_tailer` (`:898`).
- **`adopt`** is what lets children survive a daemon restart: the supervisor re-attaches to processes
  it is already running rather than duplicating them.
- **Timing constants**: `RESPAWN_DELAY` = 2s (`:40`) between a crash and respawn; `STOP_GRACE` = 10s
  (`:55`) between a graceful stop signal and force-kill; `wait_child`/adopted-monitor poll on 1s
  (`:736`, `:741`) and `stop_child` polls every 100ms (`:886`).
- **Re-exec**: `reexec_would_change_binary()` (`:220`) makes a re-exec a no-op when the binary is
  unchanged (matching the `NoChange` reply); `reexec_self()` (`:236`) replaces the supervisor process,
  after `save_state_for_reexec` has persisted the registered specs and running PIDs to
  `reexec_state_path(socket_path)` (`:35`) as `SavedState`/`SavedService` (`:22-33`). Children are
  reparented, not restarted.
- **Logs**: `log_dir(socket_path)` (`:43`) and `service_log_path(socket_path, name)` (`:51`) — per
  service; `spawn_log_tailer` streams lines back as `Notification::Log`, with `strip_ansi` (`:989`)
  cleaning escape sequences.
- **Signals**: `wait_for_shutdown_signal()` has a unix and a non-unix implementation (`:187`, `:211`).
- **Tests**: an in-file integration suite (`:978` onward) — `start_test_supervisor`, `setup`,
  `client`, `sleep_spec`, and cases such as `test_register_and_list`, `test_stop_keeps_registration`,
  `test_start_stopped_service`, `test_update_self_unchanged_binary_skips_reexec`. These exercise the
  compat-sensitive paths directly.

## 5. Client (`src/client.rs`, 252 lines)

The daemon-side API: `Client::connect(sock, timeout)` (`Client::connect(sock, Duration::from_secs(5))`
in tests), then request methods mirroring the protocol (`register`, `unregister`, `list`, `stop`,
`start`, `restart`, `kill`, `shutdown`, `update_self`) plus notification handling. `Client` and the
main protocol types are re-exported from the crate root (`lib.rs:16-17`) so the daemon and
`mac-mgmt-agent` depend on the crate rather than reaching into modules.

## 6. How the daemon side plugs in

- `mac-mgmt-agent` depends on `mac-mgmt-services` and owns the higher-level policy:
  `service_mgmt.rs` `ServiceManager` maps each `ManagedService` onto a `SpawnSpec` and calls
  `Register`, then polls `List`/`Services` on the health tick, compares `resolved_program` against
  the expected store path (drift detection) and schedules graceful restarts when a spec changes.
- Crash handling is split: the **supervisor** auto-restarts crashed processes after
  `RESPAWN_DELAY`, while the **daemon** deliberately does *not* re-register on crash — doing so would
  kill the process the supervisor has already restarted (`service_mgmt.rs:986-993`). The daemon's own
  exponential crash backoff (5s/10s/… via `crash_backoff_delay`) applies before a *repair + restart*
  is attempted. The one exception is a pending upgrade: a `CrashBackoff` service still has it applied
  and is reregistered onto the new binary, since a stale pin may be the cause of the crash loop and
  `check_upgrades()` will not re-install while `upgrade_pending` is set (staff ping f74b6db8).
- The daemon asks the supervisor to re-exec after a self-update so it picks up the new binary, and
  tolerates `NoChange` when it is already current.
- `mac-mgmt-agent/src/service/{systemd.rs, launchd.rs, mod.rs}` select the host's service backend
  where the supervisor is not the mechanism (units/plists for `unmanaged/` services).

## 7. Feature gating

The crate itself has no meaningful feature gates — it is always built when the daemon enables
`services` (`daemon/Cargo.toml`: `services = ["mac-mgmt-agent/services"]`, and
`mac-mgmt-agent`'s `services = ["relay"]`). In the daemon, disabling `services` removes
`managed_service`, `service_mgmt`, `services/`, `connectors/`, `config_providers` and `unmanaged`,
plus the `ai_proxy` and supervisor-related paths, and heartbeat/assessment fall back to empty service
lists (`daemon.rs` has explicit `#[cfg(not(feature = "services"))]` arms producing `Vec::new()`).
