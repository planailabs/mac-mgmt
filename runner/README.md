# mac-mgmt-runner

Orchestrator daemon that spins up a matrix of mac-mgmt test clusters on a
remote Incus host, reconciles their configurations against a mac-mgmt
server, monitors heartbeats, and periodically re-provisions random cells.

Runs as a long-lived daemon with a local HTTP API. The same binary
exposes CLI subcommands that drive the daemon over that API.

## What it does

For each configured cell of the `(agent provider) × (LLM provider) ×
(cloud provider)` matrix the runner:

1. Creates a cluster on the mac-mgmt server (scoped to one organization).
2. Uploads the matching `ClusterConfig` via the setting API.
3. Generates a fresh Ed25519 host key and derives the expected
   `instance_id` (hex SHA-256 of the SSH-wire public key) before any VM
   boots.
4. Fetches a ready-to-use cloud-init YAML from the server, embedding
   that host key so the daemon reports the predicted `instance_id`.
5. Launches an Incus container/VM with the cloud-init as `user-data`.
6. Watches for a heartbeat with the expected `instance_id`. If none
   arrives within `deploy_timeout` (default 15 minutes) the instance is
   torn down and recreated with a new host key. After
   `max_deploy_retries` the cell is parked and reported to Sentry.
7. Destroys and re-creates a random cell every
   `random_reprovision_interval` (default 30 minutes) to exercise the
   bootstrap path continuously.

Failures are logged to Sentry with the matrix cell key and attempt
count as tags.

Ollama cells default to `smollm2:1.7b` — small enough to keep the test
instances cheap, large enough to support tool calling.

## Setup

1. **mac-mgmt server** must be reachable at its **REST API URL** —
   `api.external_url` in the server config (default port 7378),
   **not** the web UI port (default 7377, OIDC-protected). The runner
   runs a preflight `GET /api/self` at startup and refuses to proceed
   if the URL redirects to an OIDC sign-in page.
2. **Admin token**: created directly in the DB or via the web UI
   (`kind = "admin"`).
3. **Pick an organization** — every matrix cluster will be created
   inside it so an existing org-scoped setting token can manage them.
4. **Incus host** with the HTTPS listener enabled and a client cert
   trusted:

   ```
   # on the Incus host
   incus config set core.https_address :8443
   incus config trust add-certificate /path/to/runner-client.crt
   ```

5. **Runner config**: copy `config.example.toml` to
   `/etc/mac-mgmt-runner/config.toml` and fill in:
   - `mgmt.url`, `mgmt.admin_token`, `mgmt.organization_id`
   - `incus.url`, `incus.client_cert`, `incus.client_key`
   - `matrix.cloud_api_keys` for each cloud provider you want included
   - optional `sentry.dsn`

## CLI

```
mac-mgmt-runner [--config PATH] <command>

daemon                 Run the orchestrator + HTTP API
status [--json]        Fleet snapshot (dials the running daemon)
provision              Force a reconcile (ensure every cell is present)
teardown --yes         Destroy every provisioned cell
reprovision [KEY]      Destroy+recreate one cell (KEY omitted = random)
matrix [--json]        Print the computed matrix without touching anything
```

Subcommands other than `daemon` and `matrix` are thin HTTP clients that
dial `http://{api.bind}:{api.port}` — default `127.0.0.1:9400`.

### Status output

```
matrix: 8 cells, 5 provisioned
  ✔ openclaw-ollama             cluster=... incus=mmr-openclaw-ollama
  ✔ openclaw-cloud-anthropic    cluster=... incus=mmr-openclaw-cloud-anthropic
  … openclaw-cloud-openai       cluster=... incus=mmr-openclaw-cloud-openai   [starting]
  ✗ none-ollama                 cluster=... incus=mmr-none-ollama             fail=1  [mgmt error: ...]
  ⛔ none-cloud-groq             cluster=... incus=mmr-none-cloud-groq         fail=3
```

- `✔` — healthy heartbeat within `heartbeat_stale_after`
- ` ` — healthy but no explicit probe data
- `…` — still inside startup grace (pregenerated `instance_id` not yet
  reported)
- `✗` — unhealthy (stale heartbeat or a failing service probe)
- `⛔` — parked: exceeded `max_deploy_retries`. Run
  `reprovision <key>` to unpark.
- `·` — not provisioned

## HTTP API

```
GET  /status
POST /provision
POST /teardown
POST /reprovision            # random
POST /reprovision/<key>      # specific cell
POST /shutdown
```

All responses are JSON. No auth — bind to `127.0.0.1` and expose via
SSH / unix firewall.

## Cloud-init endpoint (server side)

`POST /api/setting/cloud-init` is not specific to the runner — it's
usable in production whenever a fresh machine needs to bootstrap into a
cluster. The body is:

```json
{
  "system": "x86_64-linux-musl",
  "server_url": "https://mgmt.example.com",   // optional
  "daemon_version": "0.1.5",                   // optional
  "label": "prod-bootstrap-2026-04-15",        // optional
  "host_key_pem": "-----BEGIN ...",            // optional
  "instance_id": "abc123..."                   // optional, echoed back
}
```

The response is `{cloud_init, sync_token, instance_id}`. Pipe
`cloud_init` directly into any cloud provider's user-data field.
