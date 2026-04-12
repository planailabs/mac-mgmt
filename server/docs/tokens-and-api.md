# Tokens and API

mac-mgmt uses bearer tokens for API authentication. There are three token types, each with a different scope and set of permitted operations.

## Token types

### Sync tokens

Used by daemons to fetch their configuration and report status. A sync token is scoped to a single cluster.

**Permitted operations:**

- `GET /api/config` — fetch cluster configuration
- `GET /api/skills` — fetch assigned skills (Nix store paths)
- `GET /api/mcp-servers` — fetch assigned MCP servers
- `GET /api/ssh-keys` — fetch authorized SSH keys
- `GET /api/update` — check for daemon updates
- `GET /api/nixpkgs` — fetch nixpkgs pin
- `GET /api/self` — get token/cluster identity
- `POST /api/heartbeat` — send daemon heartbeat

### Setting tokens

Used for programmatic cluster configuration. Can be scoped to a single cluster or to an entire organization.

- **Single-cluster**: `cluster_id` is set when the token is created
- **Organization-scoped**: `organization_id` is set; requires `X-Cluster-Id` header on each request to specify the target cluster

**Permitted operations** (all under `/api/setting/`):

- Config: `GET`, `PUT`, `PATCH`
- Skills: `GET`, `POST`, `DELETE`, `PATCH`
- Bundles: `GET`, `POST`, `DELETE`, `PATCH`
- MCP servers: `GET`, `POST`, `DELETE`, `PATCH`
- MCP bundles: `GET`, `POST`, `DELETE`, `PATCH`
- SSH keys: `GET`, `POST`, `DELETE`
- Catalog: `GET`

### Admin tokens

Full access to all API endpoints. Not scoped to any cluster or organization. Requires `X-Cluster-Id` header when performing setting operations.

**Additional operations** (all under `/api/admin/`):

- Cluster management (CRUD)
- Token management (create, list, revoke)
- Rollout groups and rollouts
- Skill MCP dependencies
- Daemon versions

## Using the API

All API requests require a bearer token in the `Authorization` header:

```
Authorization: Bearer <token>
```

For organization-scoped setting tokens and admin tokens, specify the target cluster:

```
X-Cluster-Id: <cluster-uuid>
```

### Common endpoints

| Method | Endpoint | Token | Description |
|--------|----------|-------|-------------|
| `GET` | `/api/self` | Any | Returns token identity and cluster info |
| `GET` | `/api/config` | Sync | Get cluster config JSON |
| `PUT` | `/api/setting/config` | Setting | Replace full cluster config |
| `PATCH` | `/api/setting/config` | Setting | Update a single config key |
| `GET` | `/api/skills?arch=<system>` | Sync | Get skill slug → Nix store path map |
| `GET` | `/api/mcp-servers` | Sync | Get MCP server slug → config map |
| `POST` | `/api/heartbeat` | Sync | Send daemon heartbeat with status |

### Interactive API docs

Admin users can access the Swagger UI at the API's `/api/swagger-ui/` endpoint for a full interactive reference of all available routes.

## Token security

- Raw tokens are shown **only once** at creation time — store them securely
- Tokens are stored as SHA-256 hashes in the database
- Tokens can be revoked at any time through the web UI or API
- Daemon heartbeats include an ed25519 signature for cryptographic identity verification

## Creating tokens

Tokens can be created through:

1. **Web UI** — navigate to Admin Tokens or to a cluster's detail page
2. **API** — `POST /api/admin/tokens` (requires an admin token)
