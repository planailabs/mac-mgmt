---
audience: admin
---

# Authentifizierung

mac-mgmt verwendet OpenID Connect (OIDC) für die Benutzerauthentifizierung. Jeder standardkonforme OIDC-Anbieter funktioniert; dieser Leitfaden verwendet [Zitadel](https://zitadel.com) als empfohlenen Identity-Broker.

## Architektur

```
mac-mgmt-server --OIDC--> Zitadel --+--> OpenID Connect (Google, Supabase, ...)
                                     +--> SAML
                                     +--> LDAP
                                     +--> Other OIDC providers
```

Zitadel fungiert als Föderations-Hub: mac-mgmt benötigt nur eine einzige OIDC-Integration, während Zitadel die vorgelagerten Identitätsanbieter übernimmt (Social Logins, Enterprise-SAML/LDAP, weitere OIDC-Anbieter). Die mac-mgmt-Konfiguration bleibt dadurch einfach — unabhängig davon, wie viele Identitätsquellen Ihre Organisation nutzt.

## OIDC-Ablauf

1. Der Benutzer ruft mac-mgmt auf und wird zum Autorisierungs-Endpunkt des Anbieters umgeleitet.
2. Der Anbieter authentifiziert den Benutzer (ggf. delegiert an einen vorgelagerten IdP).
3. Bei Erfolg leitet der Anbieter mit einem Autorisierungscode zurück zu `{external_url}/auth/{slug}/callback`.
4. mac-mgmt tauscht den Code gegen ein ID-Token und extrahiert E-Mail und Namen des Benutzers.
5. Die Zugriffskontrollregeln werden ausgewertet (siehe unten).
6. Bei Erlaubnis wird eine Sitzung erstellt und der Benutzer in der Datenbank angelegt bzw. aktualisiert.

Sitzungen dauern 6 Stunden (gleitendes Fenster) und werden in Redis gespeichert, sofern konfiguriert, andernfalls in PostgreSQL.

## Konfiguration

Fügen Sie einen oder mehrere `[[auth.providers]]`-Einträge in Ihre Server-Konfiguration ein:

```toml
[auth]
cookie_secret = "generate-with-openssl-rand-hex-32"
external_url = "https://mac-mgmt.example.com"
# redis_url = "redis://localhost:6379"   # optional session cache
# admin_emails = ["admin@example.com"]   # auto-granted admin on first login

[[auth.providers]]
slug = "zitadel"
name = "Zitadel"
issuer = "https://zitadel.example.com"
client_id = "your-client-id"
client_secret = "your-client-secret"
allowed_domains = ["example.com"]
# auto_join_orgs = ["default"]
```

| Feld | Erforderlich | Beschreibung |
|-------|----------|-------------|
| `slug` | ja | URL-Pfadsegment für diesen Anbieter (`/auth/{slug}/login`) |
| `name` | ja | Anzeigename auf der Anmeldeseite |
| `issuer` | ja | OIDC-Issuer-URL (Discovery unter `{issuer}/.well-known/openid-configuration`) |
| `client_id` | ja | OAuth2-Client-ID |
| `client_secret` | ja | OAuth2-Client-Secret |
| `allow_all` | nein | Jeden authentifizierten Benutzer akzeptieren (Standard: false) |
| `allowed_domains` | nein | Allowlist für E-Mail-Domains (z. B. `["example.com"]`) |
| `allowed_emails` | nein | Allowlist für einzelne E-Mail-Adressen |
| `scopes` | nein | OAuth-Scopes (Standard: `["openid", "email", "profile"]`) |
| `auto_join_orgs` | nein | Organisationen, denen neue Benutzer automatisch beitreten (mit der Rolle Lesen) |

Mindestens eines von `allow_all`, `allowed_domains` oder `allowed_emails` muss gesetzt sein, sonst besteht kein Benutzer die Zugriffskontrolle.

## Zitadel einrichten

1. **Erstellen Sie ein Projekt** in Ihrer Zitadel-Instanz (z. B. "mac-mgmt").
2. **Erstellen Sie eine Anwendung** vom Typ "Web" mit der Authentifizierungsmethode "Code" (Authorization Code Flow).
3. **Setzen Sie die Redirect-URI** auf `{external_url}/auth/{slug}/callback` (z. B. `https://mac-mgmt.example.com/auth/zitadel/callback`).
4. **Setzen Sie die Post-Logout-URI** auf `{external_url}` (optional).
5. **Kopieren Sie Client-ID und Secret** in Ihre mac-mgmt-Konfiguration.
6. **Konfigurieren Sie vorgelagerte IdPs** in Zitadel nach Bedarf (Google, SAML, LDAP usw.) — Benutzer sehen diese Optionen auf dem Zitadel-Anmeldebildschirm.

## Zitadel-Zugriffstokens an der Management-API

Die Management-API (`/api/v1/*` und der MCP-Endpunkt `/mcp`) nimmt normalerweise die eigenen Tokens von mac-mgmt an (siehe [Token-Verwaltung](/docs/tokens-and-api)). Mit einem Abschnitt `[zitadel_bearer]` akzeptiert sie zusätzlich ein **Zitadel-Zugriffstoken eines angemeldeten Benutzers** und handelt mit genau dessen Berechtigungen — denselben Organisationen, Rollen und Admin-Rechten wie in seiner Web-Sitzung. Der Chat von plan.ai nutzt das: Sein Connector erhält ein solches Token per Zitadel-Token-Exchange und ruft `/mcp` im Namen des Benutzers auf.

```toml
[zitadel_bearer]
issuer = "https://id.plan.ai"
project_id = "397603440676569224"   # audience the token must carry
client_id = "397603441129619592"    # API application of that project
client_secret = "..."               # its secret (Basic auth for introspection)
```

- Ein Bearer-Token wird zuerst als mac-mgmt-Token gesucht; nur sonst wird es am Introspection-Endpunkt von Zitadel geprüft (Ergebnisse werden bis zu einer Minute zwischengespeichert). Ein Token ohne `project_id` in seiner Audience wird abgelehnt (401).
- Der Benutzer wird über die E-Mail-Adresse des Tokens gefunden. Es muss bereits ein mac-mgmt-Benutzer mit dieser E-Mail-Adresse existieren — Bearer-Tokens legen nie Benutzer an; eine unbekannte E-Mail-Adresse wird mit 403 beantwortet.
- Wurde das Token per Impersonation erhalten, wird das handelnde Dienstkonto (Claim `act`) festgehalten: Die Anfrage läuft als `user:<email> via <actor>` und wird im Audit-Log protokolliert.
- Ohne den Abschnitt werden nur mac-mgmt-Tokens akzeptiert.

## Mehrere Anbieter

Sie können mehrere OIDC-Anbieter gleichzeitig konfigurieren. Jeder erhält einen eigenen `[[auth.providers]]`-Eintrag mit einem eindeutigen `slug`. Die Anmeldeseite zeigt alle konfigurierten Anbieter an.

```toml
[[auth.providers]]
slug = "zitadel"
name = "Company SSO"
issuer = "https://zitadel.example.com"
client_id = "..."
client_secret = "..."
allowed_domains = ["example.com"]

[[auth.providers]]
slug = "google"
name = "Google"
issuer = "https://accounts.google.com"
client_id = "..."
client_secret = "..."
allowed_emails = ["contractor@gmail.com"]
```

## Zugriffskontrolle

Nach der Authentifizierung prüft mac-mgmt die E-Mail-Adresse des Benutzers gegen die Zugriffsregeln des Anbieters:

- **`allowed_domains`** — die E-Mail-Domain muss einer der aufgeführten Domains entsprechen.
- **`allowed_emails`** — die E-Mail-Adresse muss exakt übereinstimmen.
- **`allow_all`** — jede authentifizierte E-Mail-Adresse wird akzeptiert.

Diese Regeln werden pro Anbieter anhand des `iss`-Claims im JWT ausgewertet, sodass verschiedene Anbieter unterschiedliche Zugriffsrichtlinien haben können.

## Admin-E-Mails

E-Mail-Adressen in `auth.admin_emails` erhalten bei der ersten Anmeldung automatisch globale Admin-Rechte. Das ist nützlich für das Bootstrapping — nach der Ersteinrichtung lassen sich Admins über die Seite [Benutzerverwaltung](/docs/user-management) verwalten.

## Sitzungsspeicher

Sitzungen werden in Redis gespeichert, wenn `redis_url` konfiguriert ist, andernfalls in PostgreSQL. Für Produktionsumgebungen mit mehreren Server-Instanzen wird Redis empfohlen, damit Sitzungen über alle Replikate hinweg geteilt werden.

## Entwicklungsmodus

Setzen Sie `DEV_ONLY_NO_AUTH=1`, um die Authentifizierung vollständig zu umgehen. Dabei wird ein fest kodierter `dev@localhost`-Benutzer verwendet — niemals in Produktion einsetzen.
