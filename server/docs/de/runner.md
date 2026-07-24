---
audience: admin
---

# Fleet Runner

Der **mac-mgmt-runner** ist ein Orchestrator-Daemon, der eine Matrix von Test-Clustern auf einem entfernten [Incus](https://linuxcontainers.org/incus/)-Host hochfährt, deren Konfigurationen mit dem mac-mgmt-Server abgleicht, Heartbeats überwacht und die Flotte kontinuierlich mit Chaos-Operationen belastet.

Er läuft als langlebiger Daemon mit einer lokalen HTTP-API. Dasselbe Binary stellt außerdem CLI-Unterbefehle bereit, die den Daemon steuern.

## Was er tut

Für jede Zelle der Matrix **(Agent-Provider) × (LLM-Provider) × (Cluster-Größe)** führt der Runner Folgendes aus:

1. Erstellt einen Cluster auf dem mac-mgmt-Server, beschränkt auf eine Organisation.
2. Lädt die passende Cluster-Konfiguration über die Setting-API hoch.
3. Generiert einen frischen Ed25519-Host-Key und leitet die erwartete `instance_id` ab, bevor eine VM bootet.
4. Ruft eine einsatzbereite Cloud-init-YAML vom Server ab (via `POST /api/setting/cloud-init`), in die der Host-Key eingebettet ist, sodass der Daemon die vorhergesagte `instance_id` meldet.
5. Startet einen Incus-Container bzw. eine VM mit dem Cloud-init als User-Data.
6. Wartet auf einen Heartbeat mit der erwarteten `instance_id`. Trifft innerhalb von `deploy_timeout` (Standard 15 Minuten) keiner ein, wird die Instanz abgerissen und mit neuem Host-Key neu erstellt — mit unbegrenzten Wiederholungen.
7. Betreibt zwei Chaos-Schleifen: eine **Resource-Churn-Schleife** (alle `chaos_interval` zufällige Skills, Bundles, MCP Server, MCP Bundles installieren/deinstallieren) und eine **VM-Chaos-Schleife** (alle `vm_chaos_interval` den Energiezustand einer Instanz umschalten oder neu provisionieren).

Cluster werden automatisch in eine [Rollout-Gruppe](/docs/rollouts) `{name_prefix}fleet` aufgenommen, um sie leicht adressieren zu können.

## Voraussetzungen

| Voraussetzung | Detail |
|---|---|
| mac-mgmt-Server | Erreichbar unter seiner **REST-API-URL** (Standardport 7378), **nicht** dem Web-UI-Port |
| Admin Token | `kind = "admin"` — über die Web-UI oder direkt in der Datenbank erstellen |
| Organisation | Alle Matrix-Cluster werden innerhalb einer konfigurierten Organisation erstellt |
| Incus-Host | HTTPS-Listener aktiviert, Client-Zertifikat des Runners als vertrauenswürdig eingetragen |
| NixOS (optional) | Ein NixOS-Modul für das systemd-Deployment wird mitgeliefert |

## Einrichtung

### 1. Incus-Host

Aktivieren Sie die HTTPS-API auf dem Incus-Host und tragen Sie das Client-Zertifikat des Runners als vertrauenswürdig ein:

```
incus config set core.https_address :8443
incus config trust add-certificate /path/to/runner-client.crt
```

### 2. Konfiguration

Kopieren Sie die Beispielkonfiguration nach `/etc/mac-mgmt-runner/config.toml` (oder einen beliebigen Pfad — überschreibbar mit `--config` oder `MAC_MGMT_RUNNER_CONFIG`).

**Pflichtfelder:**

| Abschnitt | Schlüssel | Beschreibung |
|---|---|---|
| `[mgmt]` | `url` | REST-API-URL des mac-mgmt-Servers (Port 7378, **nicht** die Web-UI) |
| `[mgmt]` | `admin_token` | Admin-Bearer-Token |
| `[mgmt]` | `organization_id` | UUID der Organisation, unter der Cluster erstellt werden |
| `[incus]` | `url` | Incus-HTTPS-Endpunkt (z. B. `https://incus.example:8443`) |
| `[incus]` | `client_cert` | Pfad zum PEM-Client-Zertifikat |
| `[incus]` | `client_key` | Pfad zum PEM-Client-Key |

**Optionale mgmt-Felder:**

| Schlüssel | Standard | Beschreibung |
|---|---|---|
| `public_url` | `api.external_url` des Servers | URL, die in das Cloud-init eingebettet wird, damit sich Daemons verbinden können |
| `system` | `x86_64-linux` | Nix-System-Bezeichner für die Daemon-Download-URL |
| `daemon_version` | automatisch aufgelöst | Eine bestimmte Daemon-Version pinnen; weglassen, um sie aus Rollout/Pin/Latest aufzulösen |

### 3. Daemon starten

```
mac-mgmt-runner --config /etc/mac-mgmt-runner/config.toml daemon
```

Beim Start läuft ein Preflight-`GET /api/self`. Leitet die URL auf eine OIDC-Anmeldeseite um (Web-UI-Port), verweigert der Runner den Start mit einer klaren Fehlermeldung.

## Konfigurationsreferenz

### `[incus]`

| Schlüssel | Standard | Beschreibung |
|---|---|---|
| `project` | `"default"` | Incus-Projekt, in dem Instanzen erstellt werden |
| `image_alias` | `"ubuntu/26.04/cloud"` | Image-Alias oder -Fingerprint |
| `image_server` | `"https://images.linuxcontainers.org"` | Image-Server für Remote-Lookups |
| `instance_type` | `"container"` | `"container"` oder `"virtual-machine"` |
| `profiles` | `["default"]` | Anzuhängende Incus-Profile |
| `name_prefix` | `"mmr-"` | Präfix für Cluster- und Instanznamen |
| `server_ca` | *(keine)* | PEM-CA zur Validierung des Incus-Servers; weglassen, um die TLS-Verifikation zu überspringen |

### `[api]`

| Schlüssel | Standard | Beschreibung |
|---|---|---|
| `bind` | `"127.0.0.1"` | Bind-Adresse für die lokale HTTP-API des Runners |
| `port` | `9400` | Bind-Port |

### `[fleet]`

| Schlüssel | Standard | Beschreibung |
|---|---|---|
| `reconcile_interval` | `"1m"` | Wie oft die Reconcile-Schleife läuft |
| `vm_chaos_interval` | `"30m"` | Intervall zwischen Chaos-Operationen auf VM-Ebene (Umschalten / Neuprovisionierung) |
| `chaos_interval` | `"5m"` | Intervall zwischen Resource-Churn-Chaos-Operationen (Skills, Bundles, MCP). `"0"` oder `"off"` zum Deaktivieren |
| `state_path` | `"/var/lib/mac-mgmt-runner/state.json"` | Pfad zur persistierten Flotten-Zustandsdatei |
| `startup_grace` | `"3m"` | Instanzen, die jünger sind, gelten nicht als fehlerhaft |
| `heartbeat_stale_after` | `"5m"` | Heartbeat-Altersschwelle, ab der eine Instanz als fehlerhaft markiert wird |
| `deploy_timeout` | `"15m"` | Zeit, die eine frische Instanz für einen Heartbeat hat, bevor Abriss + Neuversuch erfolgen |
| `max_concurrent_launches` | `3` | Max. Zellen gleichzeitig in der Launching-Phase; weitere warten bei ConfigPushed |

### `[sentry]`

| Schlüssel | Standard | Beschreibung |
|---|---|---|
| `dsn` | *(keine)* | Sentry-DSN für Fehlerberichte. Weglassen zum Deaktivieren. |
| `environment` | *(keine)* | Umgebungs-Tag (z. B. `"staging"`, `"production"`) |

### `[matrix]`

| Schlüssel | Standard | Beschreibung |
|---|---|---|
| `ollama_model` | `"smollm2:1.7b"` | Modell für jede Ollama-LLM-Zelle (muss Tools unterstützen) |
| `lms_model` | `"smollm2-1.7b-instruct"` | Modell für jede LM-Studio-Zelle |
| `agents` | `["openclaw", "none"]` | Einzubeziehende Agent-Provider |
| `llms` | `["ollama", "lms", "cloud"]` | Einzubeziehende LLM-Provider |
| `cloud_providers` | alle bekannten | Einzubeziehende Cloud-Sub-Provider (nur solche mit API-Schlüsseln erzeugen Zellen) |
| `cluster_sizes` | `[1, 2]` | Knotenanzahl pro Cluster; jede Größe multipliziert die Matrix |
| `cloud_api_keys` | `{}` | Zuordnung Cloud-Provider → API-Schlüssel. Zellen werden nur für hier gelistete Provider erzeugt. |

## Zellen-Lebenszyklus (Zustandsmaschine)

Jede Matrix-Zelle durchläuft diese Phasen:

```
Pending → ClusterCreated → ConfigPushed → Launching → Running
                                            ↓ timeout
                                     ConfigPushed (retry with new host key)
```

- **Pending** — noch nichts erstellt
- **ClusterCreated** — Cluster existiert auf dem mgmt-Server
- **ConfigPushed** — Cluster-Konfiguration hochgeladen
- **Launching** — Incus-Instanzen erstellt, Warten auf Heartbeats
- **Running** — alle erwarteten Heartbeats eingetroffen

Der Zustand wird nach jedem Übergang unter `state_path` persistiert. Bei einem Absturz wird an der zuletzt aufgezeichneten Phase fortgesetzt.

## Chaos-Operationen

### Resource Churn (alle `chaos_interval`)

Wählt einen zufälligen laufenden Cluster und einen zufälligen Ressourcentyp (Skill, Bundle, MCP Server, MCP Bundle). Nutzt gewichtete Zufälligkeit: `P(install) = not_installed / total`, `P(uninstall) = installed / total`. So bleiben Cluster unabhängig von der Kataloggröße etwa halb installiert.

### VM-Chaos (alle `vm_chaos_interval`)

Wählt eine zufällige laufende Zelle und führt entweder aus:
- **Umschalten** (90 %) — fragt den Live-Incus-Zustand ab, stoppt eine laufende Instanz oder startet eine gestoppte
- **Neuprovisionierung** (10 %) — zerstört die gesamte Zelle und baut sie neu auf

Durch Chaos gestoppte Instanzen werden nach 2 Stunden automatisch neu gestartet, um längere Ausfälle zu vermeiden.

## CLI-Befehle

| Befehl | Beschreibung |
|---|---|
| `daemon` | Orchestrator-Daemon + HTTP-API ausführen |
| `status [--json]` | Flotten-Snapshot (kontaktiert den laufenden Daemon) |
| `provision` | Vollständigen Reconcile erzwingen |
| `teardown --yes` | Jede provisionierte Zelle zerstören und den Runner pausieren |
| `redeploy --yes` | Alles löschen (inkl. verwaister Cluster auf dem Server) und neu aufbauen |
| `reprovision [KEY]` | Eine Zelle zerstören + neu erstellen (zufällig, wenn KEY fehlt) |
| `gc` | Nicht verwaltete Incus-Instanzen und mgmt-Cluster mit passendem Präfix löschen |
| `chaos` | Einen Resource-Churn-Tick manuell auslösen |
| `matrix [--json]` | Die berechnete Matrix ausgeben, ohne etwas zu verändern |

Nach `teardown` pausiert der Runner — automatische Schleifen bleiben inaktiv, bis `provision`, `reprovision` oder `redeploy` aufgerufen wird.

## HTTP-API und Dashboard

Der Runner stellt unter `GET /` (Standard `http://127.0.0.1:9400/`) ein Dashboard bereit, das die Flottenübersicht, eine Tabelle pro Zelle und Aktionsschaltflächen zeigt. Es aktualisiert sich alle 5 Sekunden automatisch.

| Endpunkt | Beschreibung |
|---|---|
| `GET /` | HTML-Dashboard |
| `GET /status` | JSON-Snapshot |
| `POST /provision` | Reconcile auslösen |
| `POST /teardown` | Alle Zellen zerstören + pausieren |
| `POST /redeploy` | Löschen + neu aufbauen |
| `POST /gc` | Verwaiste Ressourcen aufräumen |
| `POST /chaos` | Einen Resource-Churn-Tick auslösen |
| `POST /reprovision` | Zufällige Zelle neu provisionieren |
| `POST /reprovision/<key>` | Bestimmte Zelle neu provisionieren |
| `POST /shutdown` | Sauberes Herunterfahren |

Die HTTP-API hat keine Authentifizierung — an `127.0.0.1` binden und über SSH oder Firewall-Regeln zugänglich machen.

## NixOS-Deployment

Der Runner liefert ein NixOS-Modul unter `nixosModules.runner` mit:

```nix
{
  services.mac-mgmt-runner = {
    enable = true;
    package = mac-mgmt-runner;
    settings = {
      mgmt = {
        url = "https://mgmt.example.com:7378";
        admin_token = "<ADMIN_TOKEN>";
        organization_id = "00000000-0000-0000-0000-000000000000";
      };
      incus = {
        url = "https://incus.example:8443";
        client_cert = "/var/lib/mac-mgmt-runner/incus-client.crt";
        client_key  = "/var/lib/mac-mgmt-runner/incus-client.key";
      };
      matrix.ollama_model = "smollm2:1.7b";
    };
    environmentFile = "/run/secrets/mac-mgmt-runner.env";
  };
}
```

Das Modul erstellt einen dedizierten Benutzer/eine Gruppe, ein Zustandsverzeichnis unter `/var/lib/mac-mgmt-runner` und eine gehärtete systemd-Unit. Secrets können über `environmentFile` übergeben werden.

## Cloud-init-Endpunkt

Der Server-Endpunkt `POST /api/setting/cloud-init` ist über den Runner hinaus nutzbar — jede Automatisierung, die eine frische Maschine in einen Cluster bootstrappen muss, kann ihn aufrufen. Details unter [API-Nutzung](/docs/api-usage).

Die Web-UI bietet zudem auf der Übersichtsseite jedes Clusters eine Schaltfläche **Cloud-init…** für die Ein-Klick-Generierung (verfügbar für jeden Benutzer mit Schreibzugriff auf den Cluster).
