# Healer MCP Server Setup

The mac-mgmt server exposes a [Streamable HTTP MCP](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports#streamable-http) endpoint at `/mcp/healer` that provides fleet healing tools and staff ping management.

## Prerequisites

- A running mac-mgmt server with the `server` feature enabled
- An admin API token (create one via the web UI or `POST /api/admin/clusters/{id}/tokens`)

## Claude Code

Add to your project's `.mcp.json` or `~/.claude/mcp.json`:

```json
{
  "mcpServers": {
    "mac-mgmt-healer": {
      "type": "streamable-http",
      "url": "https://your-server.example.com/mcp/healer",
      "headers": {
        "Authorization": "Bearer YOUR_ADMIN_TOKEN"
      }
    }
  }
}
```

Then use the `/heal-staff-pings` skill or call tools directly:

```
> list_staff_pings
> create_session --instance_id abc123 --cluster_id ...
> get_system_prompt
> fetch_logs
> ...
> end_session
```

## OpenAI Codex

Add to your `codex.json` or pass via CLI:

```json
{
  "mcpServers": {
    "mac-mgmt-healer": {
      "type": "streamable-http",
      "url": "https://your-server.example.com/mcp/healer",
      "headers": {
        "Authorization": "Bearer YOUR_ADMIN_TOKEN"
      }
    }
  }
}
```

## Available Tools

### Always available (no session required)

| Tool | Description |
|------|-------------|
| `list_instances` | List fleet instances with status, relay availability, last heartbeat |
| `create_session` | Target an instance for diagnosis/remediation (populates healer tools) |
| `end_session` | Tear down the active session |
| `get_session_info` | Show active session details |
| `get_system_prompt` | Load the healer behavioral instructions for the current session |
| `list_staff_pings` | List staff pings (filterable by cluster, instance, resolved, category) |
| `get_staff_ping` | Get a single staff ping by ID |
| `resolve_staff_ping` | Mark a staff ping as resolved |
| `unresolve_staff_ping` | Reopen a resolved staff ping |

### After `create_session` (~40 tools)

Instance interaction: `read_file`, `write_file`, `list_files`, `run_command`, `fetch_logs`, `list_file_tunnels`, `list_shell_commands`

Data queries: `get_probe_status`, `get_system_sample`, `get_inventory`, `get_probe_history`, `get_metrics`, `get_heartbeat`, `get_version_info`, `get_service_state`, `get_cluster_instances`

Session management: `pin`, `set_phase`, `name_session`, `staff_ping`, `list_staff_pings`, `check_node_online`, `wait_for_node`, `wait`, `request_assessment`

Cluster config: `get_config`, `patch_config`, `set_config`, `list_skills`, `add_skill`, `remove_skill`, `list_mcp_servers`, `add_mcp_server`, `remove_mcp_server`, `send_push`

Documentation: `read_doc`, `list_docs`, `list_builtin_skills`, `use_skill`

Cluster-wide: `fetch_cluster_logs`, `run_cluster_command`, `nix_check_upgrades`

## Authentication

The MCP server uses the same admin tokens as the REST API. The token is validated during the MCP `initialize` handshake from the `Authorization: Bearer <token>` header.

## Session Lifecycle

1. **Connect** -- MCP client establishes connection, sends `initialize` with auth header
2. **List instances** -- `list_instances` to find targets
3. **Create session** -- `create_session` mints a relay proxy token, builds instance access, populates ~40 healer tools (client receives `tools/list_changed`)
4. **Load prompt** -- `get_system_prompt` returns diagnosis/remediation instructions
5. **Diagnose & remediate** -- Use healer tools to investigate and fix issues
6. **End session** -- `end_session` clears tools (client receives `tools/list_changed`)
7. **Repeat** -- Create another session for a different instance, or disconnect
