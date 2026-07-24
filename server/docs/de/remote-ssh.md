---
audience: user
---

# Remote SSH

Mit Remote SSH öffnen Sie über den Relay-Server eine SSH-Sitzung zu jedem verwalteten Cluster, ohne direkten Netzwerkzugriff auf die Maschine zu benötigen.

## Funktionsweise

1. Der Daemon verbindet sich über eine persistente WebSocket-Verbindung mit dem Relay-Server
2. Das Relay weist jedem verbundenen Daemon einen dynamischen SSH-Port zu
3. Wenn Sie sich per SSH auf diesem Port mit dem Relay verbinden, leitet das Relay die Verbindung über den WebSocket an den eingebauten SSH-Server des Daemons weiter
4. Der Daemon authentifiziert Sie über öffentliche SSH-Schlüssel (verwaltet über die Web-UI oder lokal auf der Maschine)

## Voraussetzungen

- Für den Cluster muss `relay.url` konfiguriert sein (siehe [Konfigurationsreferenz](/docs/configuration-reference))
- Für den Cluster muss `relay.remote_ssh_enabled` auf `true` gesetzt sein, oder SSH muss zur Laufzeit über die FIFO aktiviert werden
- Ihr öffentlicher SSH-Schlüssel muss dem Cluster hinzugefügt sein (über die Web-UI oder die lokale `authorized_keys`-Datei)

## Cluster konfigurieren

Setzen Sie in der Cluster-Konfiguration den Abschnitt `relay`:

```json
{
  "relay": {
    "url": "wss://relay.plan.ai",
    "remote_ssh_enabled": true
  }
}
```

| Feld | Standard | Beschreibung |
|-------|---------|-------------|
| `url` | *none* | WebSocket-URL des Relay-Servers. Muss mit `ws://` oder `wss://` beginnen |
| `remote_ssh_enabled` | `false` | Ob SSH-Zugriff beim Start des Daemons aktiviert ist |

Nach der Konfiguration verbindet sich der Daemon automatisch mit dem Relay und registriert sich.

## SSH Keys verwalten

SSH Keys legen fest, wer sich mit einem Cluster verbinden darf. Schlüssel werden aus zwei Quellen akzeptiert:

1. **Servergestützte Schlüssel** — über die Web-UI auf der Cluster-Detailseite hinzugefügt und periodisch mit dem Daemon synchronisiert
2. **Lokale Schlüssel** — in der lokalen `authorized_keys`-Datei des Daemons auf der Maschine selbst gespeichert

Bei der Authentifizierung einer Sitzung werden beide Quellen zusammengeführt. Es wird ausschließlich Public-Key-Authentifizierung unterstützt; Passwort-Authentifizierung ist deaktiviert.

Um einen Schlüssel über die Web-UI hinzuzufügen, öffnen Sie die Detailseite eines Clusters und verwenden den Abschnitt SSH Keys.

## `relay-ssh` verwenden

Das CLI-Tool `relay-ssh` bietet eine bequeme Möglichkeit, sich zu verbinden, ohne Ports manuell nachschlagen zu müssen.

### Konfiguration

Erstellen Sie eine Konfigurationsdatei unter `~/.config/relay-ssh/config.toml`:

```toml
relay_url = "https://relay.plan.ai"
token = "your-setting-or-admin-token"
```

Alternativ verwenden Sie Umgebungsvariablen:

| Variable | Beschreibung |
|----------|-------------|
| `RELAY_URL` | URL des Relay-Servers |
| `RELAY_TOKEN` | Authentifizierungs-Token |

Oder übergeben Sie sie als CLI-Flags: `--relay` und `--token`.

Die Prioritätsreihenfolge ist: CLI-Flags > Umgebungsvariablen > Konfigurationsdatei.

### Aktive Tunnel auflisten

```
relay-ssh --list
```

Dies zeigt alle aktuell mit dem Relay verbundenen Cluster, auf die Ihr Token Zugriff hat:

```
INSTANCE       AGENT                    HOSTNAME                 CLUSTER          PORT
a1b2c3d4e5f6   my-agent                 macbook-pro              dev-cluster      30042
```

### Mit einem Cluster verbinden

Wenn nur ein Tunnel aktiv ist:

```
relay-ssh
```

Sind mehrere Tunnel aktiv, geben Sie die Instanz-ID oder ein eindeutiges Präfix an:

```
relay-ssh a1b2c3
```

Um sich als bestimmter Benutzer zu verbinden:

```
relay-ssh -u admin a1b2c3
```

`relay-ssh` ermittelt Relay-Host und SSH-Port aus der Tunnel-Liste und führt dann `ssh` mit den korrekten Argumenten aus.

### Authentifizierung

`relay-ssh` benötigt zum Auflisten der Tunnel einen **Einstellungs-** oder **Admin-Token**. Der Token dient der Authentifizierung am Relay-Endpunkt `/api/tunnels`. Einstellungs-Tokens sehen nur Tunnel der Cluster, auf die sie Zugriff haben; Admin Tokens sehen alle Tunnel.

## SSH zur Laufzeit aktivieren/deaktivieren

Der Daemon erstellt eine FIFO (Named Pipe), die Befehle zum Umschalten des SSH-Zugriffs ohne Neustart entgegennimmt:

```
echo "enable" > ~/.config/mac-mgmt/remote-ssh
echo "disable" > ~/.config/mac-mgmt/remote-ssh
```

Im deaktivierten Zustand bleibt die Relay-Verbindung aktiv (das Metrics-Proxying funktioniert weiterhin), aber neue SSH-Sitzungsanfragen werden abgelehnt.

## Metrics-Proxying

Auch bei deaktiviertem SSH leitet das Relay Prometheus-Metrics-Anfragen der verbundenen Daemons weiter. Das ermöglicht zentrales Monitoring über den `/metrics`-Endpunkt des Relays, ohne direkten Netzwerkzugriff auf jede Maschine zu benötigen. Details unter [Flotten-Monitoring](/docs/fleet-monitoring).
