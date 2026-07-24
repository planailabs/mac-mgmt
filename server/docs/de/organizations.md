---
audience: admin
---

# Organisationen

Organisationen fassen Cluster und Benutzer für die mandantenfähige Verwaltung zusammen. Die Verwaltung von Organisationen erfordert Admin-Zugriff.

## Struktur

Eine Organisation enthält:

- **Cluster** — die verwalteten Geräte, die zur Organisation gehören
- **Mitglieder** — Benutzer mit Zugriff und rollenbasierten Berechtigungen

## Mitgliederrollen

Mitglieder einer Organisation erhalten eine von drei Rollen:

| Rolle | Beschreibung |
|------|-------------|
| **Admin** | Voller Verwaltungszugriff auf die Cluster und Einstellungen der Organisation |
| **Schreiben** | Kann Cluster-Konfigurationen und -Zuweisungen ändern |
| **Lesen** | Nur-Lese-Zugriff auf die Cluster der Organisation |

## Tokens mit Organisations-Geltungsbereich

Einstellungs-Tokens können statt auf einen einzelnen Cluster auch auf eine Organisation beschränkt werden. Ein organisationsweiter Token kann jeden Cluster der Organisation verwalten, indem er bei jeder Anfrage den Header `X-Cluster-Id` angibt.

Das ist nützlich für Automatisierung, die mehrere Cluster derselben Organisation konfigurieren muss, ohne für jeden Cluster einzelne Tokens zu verwalten.

## Organisationen verwalten

1. Öffnen Sie **Organisationen** in der Admin-Navigation
2. Erstellen Sie eine neue Organisation mit einem Namen
3. Fügen Sie der Organisation Cluster hinzu
4. Laden Sie Benutzer ein und weisen Sie Rollen zu
