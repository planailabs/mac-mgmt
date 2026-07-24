---
audience: user
---

# Flottenüberwachung

mac-mgmt bietet mehrere Werkzeuge, um Zustand und Status Ihrer Flotte zu überwachen.

## Flotten-Dashboard

Die Seite **Flotte** gibt einen Überblick über alle Cluster mit den Informationen aus dem jeweils letzten Heartbeat:

- **Daemon-Version** — welche Version jeder Cluster ausführt
- **Hostname** — der Hostname der Maschine
- **Service-Status** — Zustand der verwalteten Services (Ollama, OpenClaw usw.)
- **Zuletzt gesehen** — wann sich der Daemon zuletzt gemeldet hat

## Instanzdetails

Ein Klick auf eine Instanz im Flotten-Dashboard öffnet deren Detailseite. Sie zeigt:

- **Systeminfo** — Daemon-Version, Hostname, Umgebung, Nixpkgs-Commit
- **Services** — Zustand jedes verwalteten Service mit Probe-Ergebnissen
- **Inventar** — statisches Systeminventar aus der letzten Bewertung (Betriebssystem, Hardware, installierte Pakete)
- **Sicherheitsstatus** — Sicherheitsbefunde als Bestanden/Fehlgeschlagen-Einträge
- **GPU-Details** — GPU-Inventar kombiniert mit Live-Werten zu Auslastung, Temperatur, Leistung und VRAM-Belegung aus der letzten Heartbeat-Stichprobe
- **Tunnel** — aktive Relay-Tunnel (SSH, Datei, Shell)

### Details pro Service

Wenn Services eigenes Inventar, Livestatus oder Sicherheitsbefunde melden, erscheinen diese in einem aufklappbaren Abschnitt **Details pro Service** unterhalb der Systemdaten. Jeder Service erhält ein eigenes Panel mit bis zu drei Unterabschnitten:

- **Inventar** — statische Daten pro Service (z. B. installierte Modelle, Version, Konfiguration)
- **Livestatus** — dynamische Stichproben aus dem letzten Heartbeat (z. B. geladene Modelle, aktive Sitzungen)
- **Sicherheit** — Sicherheitsbefunde pro Service (Bestanden/Fehlgeschlagen-Einträge mit Schweregrad)

## Daemon-Heartbeats

Jeder Daemon sendet periodisch einen Heartbeat an den Server. Dieser enthält:

- Instanz-ID (kryptografischer Fingerabdruck)
- Aktuelle Daemon-Version
- Hostname und Umgebung
- Zustand der Services
- Dynamische Stichproben pro Service (Ladezustand der Modelle, aktive Sitzungen usw.)

Das Heartbeat-Intervall wird über `daemon.health_interval` in der Cluster-Konfiguration gesteuert (Standard: `"1m"`).

## Prometheus-Metriken

Jeder Daemon stellt einen Prometheus-Metrics-Endpunkt auf dem in `metrics.port` konfigurierten Port bereit (Standard: `9396`).

## Benachrichtigungen

Konfigurieren Sie Apprise-Benachrichtigungs-URLs im Abschnitt `notifications` der Cluster-Konfiguration, um Alarme für folgende Ereignisse zu erhalten:

- `daemon_started` — Daemon-Prozess gestartet
- `daemon_stopped` — Daemon-Prozess gestoppt
- `service_crashed` — ein verwalteter Service ist abgestürzt
- `service_unhealthy` — Health-Check fehlgeschlagen
- `service_recovered` — zuvor fehlerhafter Service ist wieder fehlerfrei
- `upgrade_installed` — Daemon erfolgreich aktualisiert
- `upgrade_failed` — Daemon-Upgrade fehlgeschlagen

Beispiel:

```json
{
  "notifications": {
    "urls": [
      "tgram://bottoken/chatid",
      "ntfy://ntfy.example.com/fleet-alerts"
    ],
    "events": ["service_crashed", "upgrade_failed"]
  }
}
```

## Remote-Tools

Wenn `relay.url` konfiguriert ist, stehen für jede Instanz im Flotten-Dashboard mehrere Remote-Tools zur Verfügung:

- **Logs** — Service-Logs in Echtzeit streamen
- **Shell** — vordefinierte Service-Befehle ausführen
- **Dateien** — Konfigurationsdateien auf der Instanz bearbeiten
- **Healer** — KI-gestützte automatische Diagnose und Behebung

Details zu Logs, Shell und Dateien finden Sie unter [Remote-Tools](/docs/remote-tools). Zum KI-Healing-Agent siehe [Healer Agent](/docs/healer).

## Remote-SSH-Zugriff

Wenn `relay.url` konfiguriert und `relay.remote_ssh_enabled` auf `true` gesetzt ist, können Sie sich über den Relay-Server per SSH mit Clustern verbinden. Einrichtung und Nutzung siehe [Remote-SSH](/docs/remote-ssh).
