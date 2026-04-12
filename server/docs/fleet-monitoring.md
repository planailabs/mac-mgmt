# Fleet Monitoring

mac-mgmt provides several tools for monitoring the health and status of your fleet.

## Fleet dashboard

The **Fleet** page gives an overview of all clusters with their latest heartbeat information:

- **Daemon version** — which version each cluster is running
- **Hostname** — the machine's hostname
- **Services status** — health of managed services (Ollama, OpenClaw, etc.)
- **Last seen** — when the daemon last checked in

## Daemon heartbeats

Each daemon periodically sends a heartbeat to the server containing:

- Instance ID (cryptographic fingerprint)
- Current daemon version
- Hostname and environment
- Service health status (JSON)
- Ed25519 signature for identity verification

The heartbeat interval is controlled by `daemon.health_interval` in the cluster config (default: `"1m"`).

## Prometheus metrics

Each daemon exposes a Prometheus metrics endpoint on the port configured in `metrics.port` (default: `9396`).

### Relay metrics federation

If you use the relay, you can scrape `https://relay.plan.ai/metrics` to collect federated metrics from all connected daemons. Use an organization token for authentication.

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

## Remote SSH access

When `relay.url` is configured and `relay.remote_ssh_enabled` is `true`, you can SSH into clusters through the relay server. Manage authorized SSH keys per cluster through the web UI under the cluster's detail page.
