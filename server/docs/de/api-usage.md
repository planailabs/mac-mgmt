---
audience: user
---

# API-Nutzung

Jeder programmatische Zugriff auf mac-mgmt verwendet Bearer-Tokens. Dieser Leitfaden zeigt, wie Sie sich authentifizieren und die API mit Einstellungs-Tokens nutzen.

## Authentifizierung

Übergeben Sie Ihren Token im `Authorization`-Header:

```
Authorization: Bearer <token>
```

Bei organisationsweiten Tokens geben Sie den Ziel-Cluster an mit:

```
X-Cluster-Id: <cluster-uuid>
```

## Einstellungs-Tokens

Einstellungs-Tokens werden für die programmatische Cluster-Konfiguration verwendet. Sie können sein:

- **Einzel-Cluster** — auf einen bestimmten Cluster beschränkt
- **Organisationsweit** — können jeden Cluster der Organisation verwalten (erfordert den Header `X-Cluster-Id`)

### Verfügbare Endpunkte

Alle Einstellungs-Endpunkte liegen unter `/api/setting/`:

| Methode | Endpunkt | Beschreibung |
|--------|----------|-------------|
| `GET` | `/api/setting/config` | Cluster-Konfiguration abrufen |
| `PUT` | `/api/setting/config` | Gesamte Cluster-Konfiguration ersetzen |
| `PATCH` | `/api/setting/config` | Einzelnen Konfigurationsschlüssel aktualisieren |
| `GET` | `/api/setting/skills` | Zugewiesene Skills auflisten |
| `POST` | `/api/setting/skills` | Skill zuweisen |
| `DELETE` | `/api/setting/skills/<id>` | Skill-Zuweisung entfernen |
| `GET` | `/api/setting/bundles` | Zugewiesene Bundles auflisten |
| `POST` | `/api/setting/bundles` | Bundle zuweisen |
| `DELETE` | `/api/setting/bundles/<id>` | Bundle-Zuweisung entfernen |
| `GET` | `/api/setting/mcp-servers` | Zugewiesene MCP Server auflisten |
| `POST` | `/api/setting/mcp-servers` | MCP Server zuweisen |
| `DELETE` | `/api/setting/mcp-servers/<id>` | MCP Server entfernen |
| `GET` | `/api/setting/mcp-bundles` | Zugewiesene MCP Bundles auflisten |
| `POST` | `/api/setting/mcp-bundles` | MCP Bundle zuweisen |
| `DELETE` | `/api/setting/mcp-bundles/<id>` | MCP Bundle entfernen |
| `GET` | `/api/setting/ssh-keys` | SSH Keys auflisten |
| `POST` | `/api/setting/ssh-keys` | SSH Key hinzufügen |
| `DELETE` | `/api/setting/ssh-keys/<id>` | SSH Key entfernen |

## Token-Sicherheit

- Roh-Tokens werden **nur einmal** bei der Erstellung angezeigt — bewahren Sie sie sicher auf
- Tokens können jederzeit über die Web-UI widerrufen werden
- Tokens werden als SHA-256-Hashes in der Datenbank gespeichert
