---
audience: user
ordering_override: -100
---

# Erste Schritte

Willkommen bei mac-mgmt! Dieser Leitfaden hilft Ihnen beim Einstieg.

## Überblick

mac-mgmt ist ein Flottenmanagement-System für macOS- und Linux-Geräte. Es bietet:

- **Cluster-Verwaltung** — Geräte in Clustern organisieren, mit Konfiguration pro Cluster
- **Skill-Bereitstellung** — Nix-paketierte Skills auf Geräte ausrollen
- **MCP-Server-Verwaltung** — Model-Context-Protocol-Server für OpenClaw konfigurieren
- **Rollout-Steuerung** — stufenweise Rollouts mit Gruppen und Zeitplanung
- **Flotten-Monitoring** — Heartbeats, Metriken und Benachrichtigungen
- **Remote-Werkzeuge** — Logs, Shell-Befehle, Dateibearbeitung und KI-gestützte Heilung über das Relay

## Schnellstart

1. Melden Sie sich mit den SSO-Zugangsdaten Ihrer Organisation an
2. Öffnen Sie **Cluster**, um Ihre verwalteten Geräte zu sehen
3. Öffnen Sie das **Flotten**-Dashboard, um den Gerätezustand zu überwachen
4. Unter **Dokumentation** finden Sie Leitfäden zu einzelnen Themen

## Zentrale Konzepte

### Cluster

Ein Cluster steht für ein verwaltetes macOS- oder Linux-Gerät. Jeder Cluster hat eine eindeutige Kennung und kann Skills, MCP Server und Konfiguration zugewiesen bekommen. Eine Einrichtungsanleitung finden Sie unter [Cluster-Einrichtung](/docs/cluster-setup).

### Konfiguration

Jeder Cluster besitzt eine JSON-Konfiguration, die den Daemon, LLM-Anbieter, Benachrichtigungen und mehr steuert. Alle verfügbaren Einstellungen finden Sie in der [Konfigurationsreferenz](/docs/configuration-reference).

### Flotten-Monitoring

Das Flotten-Dashboard zeigt Heartbeat-Status, Daemon-Versionen und den Zustand der Services. Details zu Metriken und Benachrichtigungen finden Sie unter [Flotten-Monitoring](/docs/fleet-monitoring).
