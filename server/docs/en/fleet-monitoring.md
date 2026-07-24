---
audience: user
---

# Fleet Monitoring

mac-mgmt provides several tools for monitoring the health and status of your fleet.

## Fleet dashboard

The **Fleet** page gives an overview of all clusters with their latest heartbeat information:

- **Daemon version** — which version each cluster is running
- **Hostname** — the machine's hostname
- **Services status** — health of managed services (Ollama, OpenClaw, etc.)
- **Last seen** — when the daemon last checked in

## Instance detail

Clicking an instance on the Fleet dashboard opens its detail page, which shows:

- **System info** — daemon version, hostname, environment, nixpkgs commit
- **Services** — health status of each managed service with probe results
- **Inventory** — static system inventory from the latest assessment (OS, hardware, installed packages)
- **Security posture** — security findings displayed as pass/fail items
- **GPU details** — GPU inventory combined with live utilization, temperature, power, and VRAM usage from the latest heartbeat sample
- **Tunnels** — active relay tunnels (SSH, file, shell)

### Per-service details

When services report their own inventory, live status, or security findings, these appear in a collapsible **Per-service details** section below the system-level data. Each service gets its own panel with up to three sub-sections:

- **Inventory** — static per-service data (e.g. installed models, version, configuration)
- **Live status** — dynamic samples from the latest heartbeat (e.g. loaded models, active sessions)
- **Security** — per-service security findings (pass/fail items with severity)

## Daemon heartbeats

Each daemon periodically sends a heartbeat to the server containing:

- Instance ID (cryptographic fingerprint)
- Current daemon version
- Hostname and environment
- Service health status
- Dynamic per-service samples (model load status, active sessions, etc.)

The heartbeat interval is controlled by `daemon.health_interval` in the cluster config (default: `"1m"`).

## Prometheus metrics

Each daemon exposes a Prometheus metrics endpoint on the port configured in `metrics.port` (default: `9396`).

## Notifications

Configure Apprise notification URLs in the `notifications` section of the cluster config to receive alerts for:

- `daemon_started` — daemon process started
- `daemon_stopped` — daemon process stopped
- `service_crashed` — a managed service crashed
- `service_unhealthy` — health check failed
- `service_recovered` — previously unhealthy service is healthy again
- `upgrade_installed` — daemon successfully upgraded
- `upgrade_failed` — daemon upgrade failed

Example:

```json
{
  "notifications": {
    "urls": [
      "tgram://bottoken/chatid",
      "ntfy://ntfy.example.com/fleet-alerts"
    ],
    "events": ["service_crashed", "upgrade_failed"]
  }
}
```

## Remote tools

When `relay.url` is configured, several remote tools are available for each instance from the Fleet dashboard:

- **Logs** — stream service logs in real time
- **Shell** — run predefined service commands
- **Files** — edit configuration files on the instance
- **Healer** — AI-powered automated diagnosis and remediation

See [Remote Tools](/docs/remote-tools) for details on logs, shell, and files. See [Healer Agent](/docs/healer) for the AI healing agent.

## Remote SSH access

When `relay.url` is configured and `relay.remote_ssh_enabled` is `true`, you can SSH into clusters through the relay server. See [Remote SSH](/docs/remote-ssh) for setup and usage.
