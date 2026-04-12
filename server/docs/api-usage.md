---
audience: user
---

# API Usage

All programmatic access to mac-mgmt uses bearer tokens. This guide covers how to authenticate and use the API with setting tokens.

## Authentication

Include your token in the `Authorization` header:

```
Authorization: Bearer <token>
```

For organization-scoped tokens, specify the target cluster with:

```
X-Cluster-Id: <cluster-uuid>
```

## Setting tokens

Setting tokens are used for programmatic cluster configuration. They can be:

- **Single-cluster** — scoped to one specific cluster
- **Organization-scoped** — can manage any cluster in the organization (requires `X-Cluster-Id` header)

### Available endpoints

All setting endpoints are under `/api/setting/`:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/setting/config` | Get cluster configuration |
| `PUT` | `/api/setting/config` | Replace full cluster configuration |
| `PATCH` | `/api/setting/config` | Update a single config key |
| `GET` | `/api/setting/skills` | List assigned skills |
| `POST` | `/api/setting/skills` | Assign a skill |
| `DELETE` | `/api/setting/skills/<id>` | Remove a skill assignment |
| `GET` | `/api/setting/bundles` | List assigned bundles |
| `POST` | `/api/setting/bundles` | Assign a bundle |
| `DELETE` | `/api/setting/bundles/<id>` | Remove a bundle assignment |
| `GET` | `/api/setting/mcp-servers` | List assigned MCP servers |
| `POST` | `/api/setting/mcp-servers` | Assign an MCP server |
| `DELETE` | `/api/setting/mcp-servers/<id>` | Remove an MCP server |
| `GET` | `/api/setting/mcp-bundles` | List assigned MCP bundles |
| `POST` | `/api/setting/mcp-bundles` | Assign an MCP bundle |
| `DELETE` | `/api/setting/mcp-bundles/<id>` | Remove an MCP bundle |
| `GET` | `/api/setting/ssh-keys` | List SSH keys |
| `POST` | `/api/setting/ssh-keys` | Add an SSH key |
| `DELETE` | `/api/setting/ssh-keys/<id>` | Remove an SSH key |

## Token security

- Raw tokens are shown **only once** at creation time — store them securely
- Tokens can be revoked at any time through the web UI
- Tokens are stored as SHA-256 hashes in the database
