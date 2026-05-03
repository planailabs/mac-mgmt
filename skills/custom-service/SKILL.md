---
name: custom-service
description: Generate a [[custom-service]] TOML config block for the mac-mgmt daemon. Use when the user wants to add a new custom service, expose a tunnel, add probes, inventory, security checks, or supervised processes.
argument-hint: <service description>
---

# Custom Service Config Generator

Generate a `[[custom-service]]` TOML block for the mac-mgmt daemon config (`~/.config/mac-mgmt/config.toml`). The user describes what they want to manage and you produce a ready-to-paste config snippet.

## Step 1: Gather requirements

Ask the user (if not already clear from the argument) what the service does. Determine which capabilities are needed:

| Capability | When to include |
|---|---|
| `spawn` | User wants the daemon to start/stop a process |
| `health_check` | User wants a command-based health check (for spawned processes) |
| `tunnels` | Service has TCP ports to expose through the relay |
| `files` | Config files or directories to expose for remote editing |
| `commands` | Shell commands to run remotely (restart, reload, etc.) |
| `probes` | Periodic HTTP or exec health/functional checks |
| `inventory` | Static facts to collect (versions, paths, etc.) |
| `samples` | Dynamic metrics to collect every heartbeat (~1 min) |
| `security` | File permission or command-based security checks |

A service can be **managed** (has `spawn` — daemon supervises the process) or **integrated** (no `spawn` — capabilities only, process managed externally by systemd/launchd/etc.).

## Step 2: Generate the TOML config

Produce a `[[custom-service]]` block following this schema reference. Only include sections the user actually needs. Use comments to explain non-obvious fields.

### Minimal skeleton

```toml
[[custom-service]]
name = "my-service"
enabled = true
```

### Field reference

**Top-level fields:**
- `name` (required) — URL-safe identifier, lowercase with hyphens
- `enabled` — default `true`

**`[custom-service.spawn]`** — Supervised process:
- `command` (required) — program path or name
- `args` — list of arguments
- `[custom-service.spawn.env]` — environment variables as key-value pairs

**`[custom-service.health_check]`** — Health check command (exit 0 = healthy):
- `command` (required) — program to run
- `args` — arguments
- `timeout_secs` — default 10

**`[[custom-service.tunnels]]`** — TCP tunnel (one per port):
- `name` (required) — URL-safe tunnel name
- `host` — default `"127.0.0.1"`
- `port` (required) — TCP port number

**`[[custom-service.files]]`** — File or folder tunnel:
- `kind` (required) — `"file"` or `"folder"`
- `name` (required) — tunnel name
- `path` (required) — filesystem path
- `writable` — default `false`
- `description` — human-readable label
- `allow_write` (folder only) — glob patterns for writable files within folder
- `include` (folder only) — glob patterns to filter visible files

**`[[custom-service.commands]]`** — Remote shell command:
- `name` (required) — URL-safe identifier
- `command` (required) — program to run
- `args` — arguments
- `description` — what it does
- `timeout_secs` — default 300
- `[custom-service.commands.arg_template]` — user-provided argument:
  - `label` — UI label
  - `placeholder` — hint text
  - `validation` — regex pattern

**`[[custom-service.probes]]`** — Periodic health probe:
- `name` (required) — probe identifier
- `kind` — `"liveness"` (default) or `"functional"`
- HTTP probe: `[custom-service.probes.http]`
  - `url` (required), `method` (default `"GET"`), `expected_status` (default `200`), `body`, `headers` (list of `["key", "value"]`), `timeout_secs` (default `10`)
- Exec probe: `[custom-service.probes.exec]`
  - `command` (required), `args`, `timeout_secs` (default `10`)

**`[[custom-service.inventory]]`** — Static facts (collected every 6h):
- `id` (required) — machine key
- `name` (required) — display label
- `value_type` — `"string"` (default), `"number"`, `"bool"`, `"json"`
- `static_value` — literal value (no command needed)
- `[custom-service.inventory.command]` — dynamic value:
  - `run` (required), `args`, `parse_regex` (first capture group used), `timeout_secs` (default `30`)

**`[[custom-service.samples]]`** — Dynamic metrics (collected every heartbeat):
- Same schema as `inventory`

**`[[custom-service.security]]`** — Security checks (evaluated every 6h):
- `id` (required) — identifier
- `severity` — `"info"`, `"low"`, `"medium"` (default), `"high"`, `"critical"`
- `message` (required) — what is being checked
- `[custom-service.security.exec]` — command check (exit 0 = pass):
  - `command` (required), `args`, `timeout_secs` (default `30`)
- `[custom-service.security.file_check]` — file check:
  - `path` (required), `exists` (default `true`), `max_mode` (octal, e.g. `"0640"`), `owner` (username)

## Step 3: Validate the output

Before presenting the config to the user, verify:

1. `name` is URL-safe (lowercase alphanumeric + hyphens only)
2. Tunnel names are unique across all services
3. If `spawn` is present and the command isn't an absolute path, note that it must be in `$PATH`
4. Probe URLs use `127.0.0.1` (not `localhost`) for consistency
5. File paths are absolute
6. `parse_regex` patterns have exactly one capture group
7. `max_mode` values are valid octal (e.g. `"0640"`, not `640`)

## Step 4: Present to the user

Show the complete TOML block in a code fence. Tell the user to append it to `~/.config/mac-mgmt/config.toml` (or the relevant cluster config on the server). Mention that the daemon picks up config changes automatically on the next health interval (~1 min).

## Examples

### Managed web app with tunnel and probe

```toml
[[custom-service]]
name = "my-webapp"
enabled = true

[custom-service.spawn]
command = "/opt/my-webapp/server"
args = ["--port=3000"]
[custom-service.spawn.env]
NODE_ENV = "production"

[custom-service.health_check]
command = "curl"
args = ["-sf", "http://127.0.0.1:3000/health"]

[[custom-service.tunnels]]
name = "my-webapp-http"
port = 3000

[[custom-service.probes]]
name = "my-webapp-health"
kind = "liveness"
[custom-service.probes.http]
url = "http://127.0.0.1:3000/health"
expected_status = 200
```

### Integrated systemd service with inventory and security

```toml
[[custom-service]]
name = "postgres"
enabled = true

[[custom-service.tunnels]]
name = "postgres-tcp"
port = 5432

[[custom-service.commands]]
name = "postgres-reload"
command = "systemctl"
args = ["reload", "postgresql"]
description = "Reload PostgreSQL configuration"

[[custom-service.files]]
kind = "file"
name = "pg-hba"
path = "/etc/postgresql/16/main/pg_hba.conf"
writable = true
description = "Client authentication config"

[[custom-service.inventory]]
id = "pg_version"
name = "PostgreSQL Version"
value_type = "string"
[custom-service.inventory.command]
run = "psql"
args = ["--version"]
parse_regex = "(\\d+\\.\\d+)"

[[custom-service.samples]]
id = "pg_connections"
name = "Active Connections"
value_type = "number"
[custom-service.samples.command]
run = "psql"
args = ["-t", "-c", "SELECT count(*) FROM pg_stat_activity WHERE state = 'active';"]
parse_regex = "(\\d+)"

[[custom-service.security]]
id = "pg_hba_perms"
severity = "high"
message = "pg_hba.conf should not be world-readable"
[custom-service.security.file_check]
path = "/etc/postgresql/16/main/pg_hba.conf"
exists = true
max_mode = "0640"
owner = "postgres"
```
