---
audience: admin
---

# Token Management

This guide covers all token types and admin API operations. For basic API usage with setting tokens, see [API Usage](/docs/api-usage).

## Token types

### Sync tokens

Used by daemons to fetch configuration and report status. Scoped to a single cluster.

**Permitted endpoints:**

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/config` | Fetch cluster configuration |
| `GET` | `/api/skills` | Fetch assigned skills (Nix store paths) |
| `GET` | `/api/mcp-servers` | Fetch assigned MCP servers |
| `GET` | `/api/ssh-keys` | Fetch authorized SSH keys |
| `GET` | `/api/update` | Check for daemon updates |
| `GET` | `/api/nixpkgs` | Fetch nixpkgs pin |
| `GET` | `/api/self` | Get token/cluster identity |
| `POST` | `/api/heartbeat` | Send daemon heartbeat |

### Setting tokens

Used for programmatic cluster configuration. Can be single-cluster or organization-scoped. See [API Usage](/docs/api-usage) for details.

### Admin tokens

Full access to all API endpoints. Not scoped to any cluster or organization. Requires `X-Cluster-Id` header when performing setting operations.

**Additional endpoints** (all under `/api/admin/`):

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/admin/clusters` | List all clusters |
| `POST` | `/api/admin/clusters/<id>/tokens` | Create cluster tokens |
| `POST` | `/api/admin/organizations/<id>/tokens` | Create org-scoped tokens |
| `POST` | `/api/admin/rollout-groups` | Create rollout groups |
| `GET` | `/api/admin/rollout-groups` | List rollout groups |
| `POST` | `/api/admin/rollouts` | Create rollouts |
| `GET` | `/api/admin/rollouts` | List rollouts |
| `POST` | `/api/admin/rollouts/<id>/start` | Start a rollout |
| `POST` | `/api/admin/rollouts/<id>/advance` | Advance to next stage |
| `POST` | `/api/admin/rollouts/<id>/pause` | Pause rollout |
| `POST` | `/api/admin/rollouts/<id>/resume` | Resume rollout |
| `POST` | `/api/admin/rollouts/<id>/complete` | Complete rollout |
| `DELETE` | `/api/admin/rollouts/<id>` | Delete rollout |

## Creating tokens

Tokens can be created through:

1. **Web UI** — navigate to **Admin Tokens** or to a cluster's detail page
2. **API** — `POST /api/admin/clusters/<id>/tokens` (requires an admin token)

## Token security

- Raw tokens are shown **only once** at creation time
- Tokens are stored as SHA-256 hashes in the database
- Tokens can be revoked at any time through the web UI or API
- Daemon heartbeats include an ed25519 signature for cryptographic identity verification

## Interactive API docs

Admin users can access the Swagger UI at the API's `/api/swagger-ui/` endpoint for a full interactive reference of all available routes.
