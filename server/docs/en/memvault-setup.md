---
audience: user
---

# Memvault Setup

Memvault is a distributed p2p memory store that lets cluster nodes share AI context (conversation history, embeddings, documents). It uses a local redb database for persistent storage and libp2p for peer-to-peer replication.

Memvault requires the daemon to be compiled with `--features memvault`.

## Enabling memvault

Set `memvault.enabled` to `true` in your cluster configuration:

```json
{
  "memvault": {
    "enabled": true
  }
}
```

With defaults, memvault will:

- Store data in `~/.local/share/memvault/`
- Listen on port `8401` for the API server
- Keep the web UI disabled
- Require no authentication

## Configuration fields

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether memvault is started |
| `data_dir` | `"~/.local/share/memvault"` | Data directory for storage (redb database, identity, cluster ID) |
| `cluster_id` | `""` | Base58-encoded 32-byte cluster ID, or `"auto"` to generate on first run |
| `bootstrap_peers` | `[]` | libp2p multiaddrs for Kademlia bootstrap |
| `port` | `8401` | API server port (`0` to disable the API server entirely) |

## Cluster ID

The `cluster_id` identifies which set of nodes share data. Nodes with the same cluster ID form a replication group.

- **Empty string** (default): the node reads its cluster ID from `{data_dir}/cluster_id` on disk. If the file doesn't exist, a zeroed ID is used.
- **`"auto"`**: generates a random 32-byte ID on first run and persists it to disk.
- **Explicit value**: a base58-encoded 32-byte ID. Use this to ensure all nodes in a cluster share the same ID.

For multi-node clusters, set the same explicit `cluster_id` on all nodes so they recognize each other as peers.

## Bootstrapping a multi-node cluster

1. **Choose a cluster ID.** Generate one with:
   ```
   openssl rand 32 | basenc --base58
   ```

2. **Configure all nodes** with the same cluster ID and bootstrap peers:
   ```json
   {
     "memvault": {
       "enabled": true,
       "cluster_id": "<base58-cluster-id>"
     },
     "relay": {
       "url": "wss://relay.plan.ai"
     }
   }
   ```

3. **Peer discovery.** Nodes find each other through:
   - **mDNS** (enabled by default via `relay.mdns_enabled`): automatic discovery on the local network.
   - **Relay circuit relay**: nodes connected to the same relay server discover each other through the gossip protocol.
   - **Bootstrap peers**: explicit libp2p multiaddrs for initial Kademlia connections. Only needed if mDNS and relay are both unavailable.

   In most setups, enabling the relay is sufficient — no explicit bootstrap peers are needed.

4. **Verify connectivity.** Check the instance detail page in the fleet dashboard. Memvault appears as an integrated service with:
   - Health status (store file exists + API responds)
   - Block count and store size
   - Cluster ID and bootstrap peers in the inventory

## API authentication

The API server listens on localhost and is secured with a bearer token that is automatically generated on first run. The token is stored at `{data_dir}/api.token` with `0600` permissions.

The MCP server (`plan-ai-memvault`) reads this token file automatically to authenticate against the daemon's API.

## Health monitoring

The daemon runs health checks on memvault as an integrated service:

- **Sync check**: verifies `blocks.redb` exists in the data directory.
- **Async check** (when port > 0): `GET /api/v1/health` on the API server.

Health status appears in the fleet dashboard alongside other managed services.

## Data management

Memvault stores all data in the configured `data_dir`:

| File | Purpose |
|------|---------|
| `blocks.redb` | Main block store (redb database) |
| `cluster_id` | Persisted cluster ID (hex-encoded) |

To reset a node's memvault, stop the daemon and delete the data directory. The node will start fresh on next boot.

See [Configuration Reference](/docs/configuration-reference) for the full field list.
