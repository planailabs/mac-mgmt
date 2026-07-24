---
audience: admin
---

# Benutzerverwaltung

Admins können Benutzerkonten über die Seite **Benutzer** in der Admin-Seitenleiste verwalten.

## Benutzerliste

Die Benutzer-Seite zeigt alle registrierten Benutzer mit Name, E-Mail, Organisationsmitgliedschaften und Admin-Status. Über den Admin-Schalter in der Liste können Sie den Admin-Status eines Benutzers direkt umschalten.

![Benutzer-Seite](/docs-img/users-list-de.png)

## Benutzer anlegen

Klicken Sie auf **Neuer Benutzer** (① oben), um ein Benutzerkonto anzulegen. Pflichtfelder:

| Feld | Beschreibung |
|-------|-------------|
| E-Mail | Die E-Mail-Adresse des Benutzers (muss eindeutig sein) |
| Name | Anzeigename |
| Admin | Ob der Benutzer globale Admin-Rechte hat |

Geben Sie die E-Mail-Adresse (①) und den Anzeigenamen (②) ein, aktivieren Sie optional Admin, und klicken Sie dann auf **Erstellen** (③):

![Formular Neuer Benutzer](/docs-img/user-new-de.png)

Hier angelegte Benutzer können sich per SSO anmelden, sofern ihre E-Mail-Adresse mit dem SSO-Identitätsanbieter übereinstimmt.

## Benutzerdetails

Klicken Sie auf den Namen eines Benutzers, um die Detailseite zu öffnen. Von dort können Sie:

- **Organisationsmitgliedschaften einsehen** — zu welchen Organisationen der Benutzer gehört und welche Rolle er jeweils hat
- **Zu Organisation hinzufügen** — den Benutzer mit einer bestimmten Rolle (Admin, Schreiben oder Lesen) einer Organisation zuweisen
- **Aus Organisation entfernen** — die Organisationsmitgliedschaft widerrufen
- **Admin umschalten** — globalen Admin-Status vergeben oder entziehen (sich selbst können Sie den Status nicht entziehen)
- **Benutzer löschen** — das Benutzerkonto dauerhaft entfernen (sich selbst können Sie nicht löschen)

## Rollen und Zugriff

### Globaler Admin

Globale Admins haben uneingeschränkten Zugriff auf alle Funktionen, Cluster und Organisationen. Sie können:

- Alle Cluster verwalten, unabhängig von Organisationsmitgliedschaften
- Organisationen, Skills, Bundles, MCP Server, Rollouts und Daemon-Versionen erstellen und verwalten
- Benutzerkonten und Admin Tokens verwalten
- Auf die Admin-Bereiche der Seitenleiste zugreifen

### Organisationsrollen

Benutzer ohne Admin-Rechte greifen über ihre Organisationsmitgliedschaft auf Cluster zu. Details zu den drei Organisationsrollen (Admin, Schreiben, Lesen) finden Sie unter [Organisationen](/docs/organizations).

### Identitätswechsel

Admins können die Identität eines anderen Benutzers übernehmen, um die Anwendung aus dessen Perspektive zu sehen. Das ist hilfreich beim Debuggen von Zugriffsproblemen. Während des Identitätswechsels wird oben auf der Seite ein Hinweisbanner angezeigt.
