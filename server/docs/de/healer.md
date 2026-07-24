---
audience: user
---

# Healer Agent

Der Healer ist ein KI-gestützter Agent, der Probleme auf Flotten-Instanzen automatisch diagnostiziert und behebt. Er verbindet sich über das Relay mit einer Daemon-Instanz, prüft den Zustand der Services, führt Shell-Befehle aus, liest Log-Dateien und wendet Korrekturen an.

## Eine Sitzung starten

Öffnen Sie das **Flotten**-Dashboard, klicken Sie auf eine Instanz und wählen Sie den Tab **Healer**. Die Seite zeigt, welche Services fehlerhaft sind, und listet vorherige Sitzungen für diese Instanz auf.

So starten Sie eine neue Sitzung:

1. Geben Sie optional Anweisungen ein, die das Problem oder den Untersuchungsauftrag beschreiben
2. Klicken Sie auf **Heilung starten**
3. Der Agent öffnet eine Sitzung mit Live-Streaming

Ohne Anweisungen diagnostiziert der Agent automatisch anhand des aktuellen Service-Zustands und Systemstatus.

## Sitzungs-Lebenszyklus

Jede Sitzung durchläuft eine Reihe von Zuständen:

| Zustand | Beschreibung |
|-------|-------------|
| Initialisierung | Der Agent verbindet sich und sammelt Kontext |
| Diagnose | Der Agent untersucht das Problem |
| Behebung | Der Agent wendet Korrekturen an |
| Prüfung | Der Agent bestätigt, dass die Korrektur funktioniert hat |
| Fertig | Sitzung erfolgreich abgeschlossen |
| Pausiert | Sitzung durch Benutzer oder Token-Budget pausiert |
| Fehlgeschlagen | Sitzung mit einem Fehler beendet |
| Abgebrochen | Sitzung wurde manuell abgebrochen |
| Menschliche Hilfe nötig | Der Agent kann das Problem nicht automatisch lösen |

Während einer aktiven Sitzung können Sie sie jederzeit **Pausieren**, **Fortsetzen** oder **Abbrechen**.

## Angepinnte Slots

Während der Agent arbeitet, pinnt er strukturierte Zusammenfassungen an den Anfang der Sitzungsansicht:

- **Diagnose** — was der Agent als Problem identifiziert hat
- **Behebungsplan** — was der Agent zu tun beabsichtigt
- **Abschlussbericht** — Zusammenfassung der durchgeführten Aktionen und des Ergebnisses

Diese Pins geben einen schnellen Überblick, ohne die vollständige Konversation lesen zu müssen.

## Mitarbeiter-Pings

Stößt der Healer auf ein Problem, das er nicht automatisch lösen kann, erstellt er einen **Mitarbeiter-Ping** — eine handlungsrelevante Benachrichtigung für einen menschlichen Operator. Mitarbeiter-Pings sind kategorisiert:

| Kategorie | Typische Ursache |
|----------|---------------|
| `hardware` | Hardware-Ausfall erkannt |
| `network` | Problem mit der Netzwerkverbindung |
| `disk_space` | Festplatte voll oder fast voll |
| `config_error` | Ungültige Konfiguration |
| `service_crash` | Wiederholte Service-Abstürze |
| `model_issue` | Fehler beim Download oder Betrieb eines LLM-Modells |
| `permission` | Berechtigungsproblem bei Dateien oder Prozessen |
| `dependency` | Fehlende Abhängigkeit |
| `security` | Sicherheitsrelevantes Anliegen |
| `performance` | Leistungsabfall |

Mitarbeiter-Pings erscheinen direkt in der Healer-Sitzungsansicht und werden zusätzlich auf der zentralen Seite **Mitarbeiter-Pings** gesammelt (erreichbar über die Admin-Seitenleiste). An beiden Stellen können Sie einen Ping als gelöst markieren.

## Vergangene Sitzungen ansehen

Die Healer-Seite jeder Instanz listet die 20 neuesten Sitzungen mit ihrem Endzustand und Erstellungszeitpunkt auf. Klicken Sie auf eine Sitzung, um den vollständigen Konversationsverlauf und die angepinnten Slots anzuzeigen.

## Voraussetzungen

- Die Instanz muss mit dem Relay verbunden sein (d. h. `relay.url` ist konfiguriert)
- Sie benötigen **Schreib**-Zugriff auf den Cluster der Instanz
- Auf dem Server muss das Healer-Subsystem aktiviert sein
