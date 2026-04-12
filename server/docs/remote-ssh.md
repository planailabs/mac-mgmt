---
audience: user
---

# Remote SSH

Remote SSH lets you open an SSH session to any managed cluster through the relay server, without requiring direct network access to the machine.

## How it works

1. The daemon connects to the relay server over a persistent WebSocket
2. The relay assigns a dynamic SSH port for each connected daemon
3. When you SSH to the relay on that port, the relay bridges the connection through the WebSocket to the daemon's built-in SSH server
4. The daemon authenticates you using SSH public keys (managed via the web UI or locally on the machine)

## Prerequisites

- The cluster must have `relay.url` configured (see [Configuration Reference](/docs/configuration-reference))
- The cluster must have `relay.remote_ssh_enabled` set to `true`, or SSH must be enabled at runtime via the FIFO
- Your SSH public key must be added to the cluster (via the web UI or the local `authorized_keys` file)

## Configuring the cluster

In the cluster configuration, set the `relay` section:

```json
{
  "relay": {
    "url": "wss://relay.plan.ai",
    "remote_ssh_enabled": true
  }
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `url` | *none* | Relay server WebSocket URL. Must start with `ws://` or `wss://` |
| `remote_ssh_enabled` | `false` | Whether SSH access is enabled when the daemon starts |

Once configured, the daemon will automatically connect to the relay and register itself.

## Managing SSH keys

SSH keys authorize who can connect to a cluster. Keys are accepted from two sources:

1. **Server-managed keys** — added through the web UI on the cluster detail page, synced to the daemon periodically
2. **Local keys** — stored in the daemon's local `authorized_keys` file on the machine itself

Both sources are merged when authenticating a session. Only public-key authentication is supported; password authentication is disabled.

To add a key through the web UI, navigate to a cluster's detail page and use the SSH Keys section.

## Using `relay-ssh`

The `relay-ssh` CLI tool provides a convenient way to connect without having to look up ports manually.

### Configuration

Create a config file at `~/.config/relay-ssh/config.toml`:

```toml
relay_url = "https://relay.plan.ai"
token = "your-setting-or-admin-token"
```

Alternatively, use environment variables:

| Variable | Description |
|----------|-------------|
| `RELAY_URL` | Relay server URL |
| `RELAY_TOKEN` | Authentication token |

Or pass them as CLI flags: `--relay` and `--token`.

The priority order is: CLI flags > environment variables > config file.

### Listing active tunnels

```
relay-ssh --list
```

This shows all clusters currently connected to the relay that your token has access to:

```
INSTANCE       AGENT                    HOSTNAME                 CLUSTER          PORT
a1b2c3d4e5f6   my-agent                 macbook-pro              dev-cluster      30042
```

### Connecting to a cluster

If only one tunnel is active:

```
relay-ssh
```

If multiple tunnels are active, specify the instance ID or a unique prefix:

```
relay-ssh a1b2c3
```

To connect as a specific user:

```
relay-ssh -u admin a1b2c3
```

`relay-ssh` resolves the relay host and SSH port from the tunnel list, then executes `ssh` with the correct arguments.

### Authentication

`relay-ssh` requires a **setting** or **admin** token to list tunnels. The token is used to authenticate with the relay's `/api/tunnels` endpoint. Setting tokens only see tunnels for clusters they have access to; admin tokens see all tunnels.

## Enabling/disabling SSH at runtime

The daemon creates a FIFO (named pipe) that accepts commands to toggle SSH access without restarting:

```
echo "enable" > ~/.config/mac-mgmt/remote-ssh
echo "disable" > ~/.config/mac-mgmt/remote-ssh
```

When disabled, the relay connection stays active (metrics proxying still works) but new SSH session requests are rejected.

## Metrics proxying

Even when SSH is disabled, the relay proxies Prometheus metrics requests from connected daemons. This enables centralized monitoring via the relay's `/metrics` endpoint without requiring direct network access to each machine. See [Fleet Monitoring](/docs/fleet-monitoring) for details.
