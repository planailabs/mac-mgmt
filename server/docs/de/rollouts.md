---
audience: admin
---

# Rollouts

Rollouts ermöglichen gestufte, kontrollierte Updates über Ihre gesamte Flotte. Damit verteilen Sie Daemon-Versions-Upgrades und Änderungen am Nixpkgs-Pin nacheinander an Gruppen von Clustern. Die Verwaltung von Rollouts erfordert Admin-Zugriff.

## Konzepte

### Rollout-Gruppen

Eine Rollout-Gruppe ist eine benannte Menge von Clustern. Erstellen Sie Gruppen für Deployment-Stufen wie:

- **Canary** — eine einzelne Testmaschine
- **Staging** — interne oder risikoarme Cluster
- **Produktion** — die gesamte Flotte

Gruppen verwalten Sie unter **Rollout-Gruppen** in der Navigation. Jede Gruppe hat einen Namen, eine Beschreibung und eine Menge von Mitglieds-Clustern.

### Rollouts

Ein Rollout definiert:

- Eine **Zielversion** — die Daemon-Version, auf die die Cluster aktualisieren sollen
- Einen **Nixpkgs-Commit** (optional) — Cluster auf eine bestimmte Nixpkgs-Revision pinnen
- Eine oder mehrere **Stufen** — jede zielt auf eine Rollout-Gruppe und wird der Reihe nach ausgeführt

### Stufen

Jede Stufe zielt auf eine Rollout-Gruppe und hat einen Status:

- `rolling` — wird aktiv an die Cluster dieser Gruppe ausgerollt
- `paused` — Ausrollen angehalten; kann fortgesetzt werden
- `completed` — alle Cluster dieser Gruppe wurden aktualisiert

## Ablauf

1. **Rollout-Gruppen erstellen** — Cluster in Deployment-Stufen organisieren
2. **Rollout erstellen** — Zielversion und/oder Nixpkgs-Commit festlegen, Stufen in Reihenfolge hinzufügen
3. **Rollout starten** — die erste Stufe beginnt mit dem Ausrollen
4. **Überwachen** — den Fortschritt auf der Rollout-Detailseite verfolgen
5. **Weiterschalten** — sieht die aktuelle Stufe gut aus, zur nächsten Stufe weiterschalten
6. **Pausieren/Fortsetzen** — bei Problemen das Ausrollen pausieren und nach der Behebung fortsetzen
7. **Abschließen** — das Rollout als abgeschlossen markieren, wenn alle Stufen fertig sind

## Funktionsweise

Wenn ein Daemon nach Updates fragt (`GET /api/update`), prüft der Server:

1. Gibt es ein aktives Rollout mit einer `rolling`-Stufe, die diesen Cluster einschließt?
2. Falls ja, wird die Zielversion des Rollouts zurückgegeben
3. Falls nein, greift die gepinnte Version des Clusters
4. Gibt es keine gepinnte Version, greift die neueste Daemon-Version

Dieselbe Rangfolge gilt für Nixpkgs-Pins (`GET /api/nixpkgs`).

## Bewährte Vorgehensweisen

- Beginnen Sie mit einer kleinen Canary-Gruppe, um Probleme früh zu erkennen
- Steuern Sie mit `upgrade_window` in der Daemon-Konfiguration, wann Upgrades stattfinden
- Beobachten Sie während eines Rollouts das Flotten-Dashboard auf fehlerhafte Services
- Pausieren Sie sofort, wenn eine Stufe vor dem Weiterschalten Probleme zeigt
