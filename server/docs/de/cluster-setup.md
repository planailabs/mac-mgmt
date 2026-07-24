---
audience: user
---

# Cluster-Einrichtung

## Einen Cluster anlegen

Öffnen Sie **Cluster** und klicken Sie auf **Neuer Cluster**. Geben Sie einen Namen ein (①) und klicken Sie auf **Erstellen** (②):

![Formular Neuer Cluster](/docs-img/cluster-new-de.png)

Der neue Cluster erscheint in der Cluster-Liste; öffnen Sie seine Detailseite, um Skills, MCP-Server und die unten beschriebene Konfiguration zuzuweisen.

## OpenClaw eigenständig einrichten (nicht vom Daemon verwaltet)

- Installieren Sie OpenClaw wie auf der OpenClaw-Website beschrieben
- Lassen Sie `openclaw.enabled` auf `false` (Standard) oder setzen Sie `global.default_agent` auf `none`

## OpenClaw über den Daemon einrichten

- Setzen Sie `openclaw.enabled` auf `true` und `global.default_agent` auf `openclaw`
- Setzen Sie `global.default_llm` auf einen Anbieter Ihrer Wahl und aktivieren Sie ihn (z. B. `ollama.enabled = true`)
  - Sie können Cloud-Anbieter-Einträge unter der `cloud`-Liste hinzufügen, um externe Cloud-Dienste für OpenClaw zu nutzen

## Nur den Modell-Anbieter einrichten

- Lassen Sie `openclaw.enabled` auf `false` (Standard) oder setzen Sie `global.default_agent` auf `none`
- Setzen Sie `global.default_llm` auf einen Anbieter Ihrer Wahl und aktivieren Sie ihn (z. B. `ollama.enabled = true`)
- Hinweis: Ohne Agent hat `default_model` keine Wirkung; es werden nur Modelle heruntergeladen und verwaltet sowie der Ollama-Server gestartet

## Einen Cluster produktionsreif machen

- Setzen Sie `relay.url` auf `wss://relay.plan.ai` oder auf Ihr Firmen-Relay, falls von uns bereitgestellt
- Fügen Sie `https://relay.plan.ai/metrics` zu Ihrem Prometheus-Monitoring hinzu
  - Verwenden Sie dafür einen Organisations-Token
