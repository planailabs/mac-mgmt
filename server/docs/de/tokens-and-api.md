---
audience: admin
---

# Token-Verwaltung

Dieser Leitfaden behandelt alle Token-Typen und Admin-API-Operationen. Für die grundlegende API-Nutzung mit Einstellungs-Tokens siehe [API-Nutzung](/docs/api-usage).

## Token-Typen

### Sync Tokens

Werden von Daemons verwendet, um Konfiguration abzurufen und Status zu melden. Auf einen einzelnen Cluster beschränkt.

**Erlaubte Endpunkte:**

| Methode | Endpunkt | Beschreibung |
|--------|----------|-------------|
| `GET` | `/api/config` | Cluster-Konfiguration abrufen |
| `GET` | `/api/skills` | Zugewiesene Skills abrufen (Nix-Store-Pfade) |
| `GET` | `/api/mcp-servers` | Zugewiesene MCP Server abrufen |
| `GET` | `/api/ssh-keys` | Autorisierte SSH Keys abrufen |
| `GET` | `/api/update` | Auf Daemon-Updates prüfen |
| `GET` | `/api/nixpkgs` | Nixpkgs-Pin abrufen |
| `GET` | `/api/self` | Token-/Cluster-Identität abrufen |
| `POST` | `/api/heartbeat` | Daemon-Heartbeat senden |

### Einstellungs-Tokens

Werden für die programmatische Cluster-Konfiguration verwendet. Können auf einen einzelnen Cluster oder auf eine Organisation beschränkt sein. Details siehe [API-Nutzung](/docs/api-usage).

### Admin Tokens

Voller Zugriff auf alle API-Endpunkte. Nicht auf einen Cluster oder eine Organisation beschränkt. Erfordern den Header `X-Cluster-Id` bei Einstellungs-Operationen.

**Zusätzliche Endpunkte** (alle unter `/api/admin/`):

| Methode | Endpunkt | Beschreibung |
|--------|----------|-------------|
| `GET` | `/api/admin/clusters` | Alle Cluster auflisten |
| `POST` | `/api/admin/clusters/<id>/tokens` | Cluster-Tokens erstellen |
| `POST` | `/api/admin/organizations/<id>/tokens` | Organisationsweite Tokens erstellen |
| `POST` | `/api/admin/rollout-groups` | Rollout-Gruppen erstellen |
| `GET` | `/api/admin/rollout-groups` | Rollout-Gruppen auflisten |
| `POST` | `/api/admin/rollouts` | Rollouts erstellen |
| `GET` | `/api/admin/rollouts` | Rollouts auflisten |
| `POST` | `/api/admin/rollouts/<id>/start` | Rollout starten |
| `POST` | `/api/admin/rollouts/<id>/advance` | Zur nächsten Stufe wechseln |
| `POST` | `/api/admin/rollouts/<id>/pause` | Rollout pausieren |
| `POST` | `/api/admin/rollouts/<id>/resume` | Rollout fortsetzen |
| `POST` | `/api/admin/rollouts/<id>/complete` | Rollout abschließen |
| `DELETE` | `/api/admin/rollouts/<id>` | Rollout löschen |

## Tokens erstellen

Tokens können erstellt werden über:

1. **Web-UI** — unter **Admin Tokens** oder auf der Detailseite eines Clusters
2. **API** — `POST /api/admin/clusters/<id>/tokens` (erfordert einen Admin Token)

Geben Sie auf der Seite **Admin Tokens** optional eine Bezeichnung und ein Ablaufdatum ein und klicken Sie dann auf **Admin Token erstellen** (①). Der Roh-Token wird nur einmal angezeigt — kopieren Sie ihn sofort:

![Admin-Tokens-Seite](/docs-img/admin-tokens-de.png)

## Token-Sicherheit

- Roh-Tokens werden **nur einmal** bei der Erstellung angezeigt
- Tokens werden als SHA-256-Hashes in der Datenbank gespeichert
- Tokens können jederzeit über die Web-UI oder die API widerrufen werden
- Daemon-Heartbeats enthalten eine ed25519-Signatur zur kryptografischen Identitätsprüfung

## Interaktive API-Dokumentation

Admin-Benutzer können die Swagger UI unter dem Endpunkt `/api/swagger-ui/` der API aufrufen — eine vollständige interaktive Referenz aller verfügbaren Routen.
