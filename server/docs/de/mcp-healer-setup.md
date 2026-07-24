# Healer-MCP-Server einrichten

Der mac-mgmt-Server stellt unter `/mcp/healer` einen [Streamable-HTTP-MCP](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports#streamable-http)-Endpunkt bereit, der Werkzeuge zur Flottenheilung und die Verwaltung von Mitarbeiter-Pings bietet.

## Voraussetzungen

- Ein laufender mac-mgmt-Server mit aktiviertem `server`-Feature
- Ein Admin-API-Token (über die Web-UI oder `POST /api/admin/clusters/{id}/tokens` erstellen)

## Claude Code

### Über die CLI

```bash
claude mcp add --transport http \
  mac-mgmt-healer \
  https://your-server.example.com/mcp/healer \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN"
```

### Über die Konfigurationsdatei

Fügen Sie Folgendes zur `.mcp.json` Ihres Projekts oder zu `~/.claude/mcp.json` hinzu:

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

### Verwendung

Verwenden Sie den `/heal-staff-pings`-Skill oder rufen Sie die Tools direkt auf:

```
> list_staff_pings
> create_session --instance_id abc123 --cluster_id ...
> get_system_prompt
> fetch_logs
> ...
> end_session
```

## OpenAI Codex

Fügen Sie Folgendes zu Ihrer `codex.json` hinzu oder übergeben Sie es per CLI:

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

## Verfügbare Tools

### Immer verfügbar (keine Sitzung erforderlich)

| Tool | Beschreibung |
|------|-------------|
| `list_instances` | Flotten-Instanzen mit Status, Relay-Verfügbarkeit und letztem Heartbeat auflisten |
| `create_session` | Eine Instanz für Diagnose/Behebung anvisieren (aktiviert die Healer-Tools) |
| `end_session` | Die aktive Sitzung beenden |
| `get_session_info` | Details der aktiven Sitzung anzeigen |
| `get_system_prompt` | Die Verhaltensanweisungen des Healers für die aktuelle Sitzung laden |
| `list_staff_pings` | Mitarbeiter-Pings auflisten (filterbar nach Cluster, Instanz, gelöst, Kategorie) |
| `get_staff_ping` | Einen einzelnen Mitarbeiter-Ping per ID abrufen |
| `resolve_staff_ping` | Einen Mitarbeiter-Ping als gelöst markieren |
| `unresolve_staff_ping` | Einen gelösten Mitarbeiter-Ping wieder öffnen |

### Nach `create_session` (~40 Tools)

Instanz-Interaktion: `read_file`, `write_file`, `list_files`, `run_command`, `fetch_logs`, `list_file_tunnels`, `list_shell_commands`

Datenabfragen: `get_probe_status`, `get_system_sample`, `get_inventory`, `get_probe_history`, `get_metrics`, `get_heartbeat`, `get_version_info`, `get_service_state`, `get_cluster_instances`

Sitzungsverwaltung: `pin`, `set_phase`, `name_session`, `staff_ping`, `list_staff_pings`, `check_node_online`, `wait_for_node`, `wait`, `request_assessment`

Cluster-Konfiguration: `get_config`, `patch_config`, `set_config`, `list_skills`, `add_skill`, `remove_skill`, `list_mcp_servers`, `add_mcp_server`, `remove_mcp_server`, `send_push`

Dokumentation: `read_doc`, `list_docs`, `list_builtin_skills`, `use_skill`

Clusterweit: `fetch_cluster_logs`, `run_cluster_command`, `nix_check_upgrades`

## Authentifizierung

Der MCP-Server verwendet dieselben Admin Tokens wie die REST-API. Der Token wird während des MCP-`initialize`-Handshakes aus dem Header `Authorization: Bearer <token>` validiert.

## Sitzungs-Lebenszyklus

1. **Verbinden** -- Der MCP-Client baut die Verbindung auf und sendet `initialize` mit dem Auth-Header
2. **Instanzen auflisten** -- `list_instances`, um Ziele zu finden
3. **Sitzung erstellen** -- `create_session` erzeugt einen Relay-Proxy-Token, richtet den Instanzzugriff ein und aktiviert ~40 Healer-Tools (der Client erhält `tools/list_changed`)
4. **Prompt laden** -- `get_system_prompt` liefert die Anweisungen für Diagnose/Behebung
5. **Diagnostizieren & beheben** -- Mit den Healer-Tools Probleme untersuchen und beheben
6. **Sitzung beenden** -- `end_session` entfernt die Tools (der Client erhält `tools/list_changed`)
7. **Wiederholen** -- Eine weitere Sitzung für eine andere Instanz erstellen oder die Verbindung trennen
