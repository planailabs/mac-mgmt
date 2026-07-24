---
audience: admin
---

# Skills und Bundles

Skills und Bundles sind der primäre Weg, Software auf Cluster auszurollen. Die Verwaltung von Skills, Bundles, MCP Servern und deren Zuweisungen erfordert Admin-Zugriff.

## Skills

Ein Skill ist ein mit Nix paketiertes Tool oder eine Anwendung. Jeder Skill hat:

- Einen eindeutigen **Slug** (Bezeichner)
- Einen lesbaren **Namen** und eine **Beschreibung**
- Einen oder mehrere **Channels** (z. B. `stable`, `unstable`), die verschiedene Release-Tracks darstellen

Wird ein Skill einem Cluster zugewiesen, löst der Daemon den Nix-Store-Pfad des Skills auf und installiert ihn.

### Skills zuweisen

Skills können einem Cluster auf zwei Arten zugewiesen werden:

1. **Direkt** — einen bestimmten Skill Channel dem Cluster zuweisen
2. **Über ein Bundle** — ein Bundle zuweisen, das den Skill enthält

Direkte Zuweisungen haben Vorrang vor Bundle-Zuweisungen. Erscheint derselbe Skill sowohl direkt als auch über ein Bundle, gewinnt die direkte Zuweisung.

## Bundles

Ein Bundle fasst mehrere Skill Channels für die bequeme Zuweisung zusammen. Statt 10 Skills einzeln jedem Cluster zuzuweisen, erstellen Sie ein Bundle und weisen dieses zu.

Bundles haben:

- Einen eindeutigen **Slug**
- Einen **Namen** und eine **Beschreibung**
- Eine Liste von **Skill Channels** (Bundle-Einträge)

## MCP Server

MCP-Server (Model Context Protocol) stellen Integrationen und Tools bereit, die OpenClaw nutzen kann. Jeder MCP Server hat:

- Einen eindeutigen **Slug**
- Eine JSON-**Konfiguration** (wird dem MCP Server beim Start übergeben)
- Eine Liste von **Nix-Paketen**, die zum Ausführen des Servers benötigt werden

### MCP Bundles

Analog zu Skill-Bundles fassen MCP Bundles mehrere MCP Server für die Sammelzuweisung zusammen.

### Transitive Abhängigkeiten

Skill Channels können MCP-Server-Abhängigkeiten deklarieren. Wird ein Skill einem Cluster zugewiesen, werden seine MCP-Server-Abhängigkeiten automatisch mit einbezogen. Diese transitiven Abhängigkeiten haben die niedrigste Priorität:

1. **Direkte Zuweisung** (höchste Priorität)
2. **Bundle-Zuweisung**
3. **Transitive Abhängigkeit** (niedrigste Priorität)

Ist ein MCP Server bereits direkt oder über ein Bundle zugewiesen, wird die transitive Abhängigkeit übersprungen.

## Sichtbarkeit im Katalog

Sowohl Skills als auch MCP Server können als **im öffentlichen Katalog ausgeblendet** markiert werden. Versteckte Einträge bleiben funktionsfähig, erscheinen aber für Nicht-Admin-Benutzer nicht in Kataloglisten.

## Synchronisierung von xzar

Skills und ihre Channels können von einem externen xzar-Dienst synchronisiert werden. Der Sync-Prozess:

1. Ruft alle Pins von xzar ab, die dem Muster `skill/{slug}/{channel}/{arch}` entsprechen
2. Erstellt fehlende Skills und Channels
3. Entfernt Channels und Skills, die in xzar nicht mehr existieren

Verwenden Sie die Schaltfläche **Von xzar synchronisieren** auf der Skills-Seite, um dies auszulösen.
