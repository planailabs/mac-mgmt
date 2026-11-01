---
audience: admin
---

# Authentication

mac-mgmt uses OpenID Connect (OIDC) for user authentication. Any standards-compliant OIDC provider works, but this guide uses [Zitadel](https://zitadel.com) as the recommended identity broker.

## Architecture

```
mac-mgmt-server --OIDC--> Zitadel --+--> OpenID Connect (Google, Supabase, ...)
                                     +--> SAML
                                     +--> LDAP
                                     +--> Other OIDC providers
```

Zitadel acts as a federation hub: mac-mgmt only needs a single OIDC integration, while Zitadel handles upstream identity providers (social logins, enterprise SAML/LDAP, additional OIDC providers). This keeps the mac-mgmt configuration simple regardless of how many identity sources your organization uses.

## OIDC flow

1. User visits mac-mgmt and is redirected to the provider's authorization endpoint.
2. The provider authenticates the user (possibly delegating to an upstream IdP).
3. On success, the provider redirects back to `{external_url}/auth/{slug}/callback` with an authorization code.
4. mac-mgmt exchanges the code for an ID token and extracts the user's email and name.
5. Access control rules are evaluated (see below).
6. If allowed, a session is created and the user is upserted into the database.

Sessions last 6 hours (sliding window) and are stored in Redis if configured, otherwise PostgreSQL.

## Configuration

Add one or more `[[auth.providers]]` entries in your server config:

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

| Field | Required | Description |
|-------|----------|-------------|
| `slug` | yes | URL path segment for this provider (`/auth/{slug}/login`) |
| `name` | yes | Display name on the login page |
| `issuer` | yes | OIDC issuer URL (discovery at `{issuer}/.well-known/openid-configuration`) |
| `client_id` | yes | OAuth2 client ID |
| `client_secret` | yes | OAuth2 client secret |
| `allow_all` | no | Accept any authenticated user (default: false) |
| `allowed_domains` | no | Email domain allowlist (e.g. `["example.com"]`) |
| `allowed_emails` | no | Individual email allowlist |
| `scopes` | no | OAuth scopes (default: `["openid", "email", "profile"]`) |
| `auto_join_orgs` | no | Organizations to auto-add new users to (with Read role) |

At least one of `allow_all`, `allowed_domains`, or `allowed_emails` must be set, otherwise no user will pass access control.

## Setting up Zitadel

1. **Create a project** in your Zitadel instance (e.g. "mac-mgmt").
2. **Create an application** of type "Web" with authentication method "Code" (authorization code flow).
3. **Set the redirect URI** to `{external_url}/auth/{slug}/callback` (e.g. `https://mac-mgmt.example.com/auth/zitadel/callback`).
4. **Set the post-logout URI** to `{external_url}` (optional).
5. **Copy the client ID and secret** into your mac-mgmt config.
6. **Configure upstream IdPs** in Zitadel as needed (Google, SAML, LDAP, etc.) -- users will see these options on Zitadel's login screen.

## Zitadel access tokens on the management API

The management API (`/api/v1/*` and the MCP endpoint `/mcp`) normally takes mac-mgmt's own tokens (see [Token Management](/docs/tokens-and-api)). With a `[zitadel_bearer]` section it also accepts a **Zitadel access token of a signed-in user** and acts with exactly that user's permissions — the same organizations, roles and admin flag as the user's web session. plan.ai's chat uses this: its connector obtains such a token by Zitadel token exchange and calls `/mcp` as the user.

```toml
[zitadel_bearer]
issuer = "https://id.plan.ai"
project_id = "397603440676569224"   # audience the token must carry
client_id = "397603441129619592"    # API application of that project
client_secret = "..."               # its secret (Basic auth for introspection)
auto_create = true                  # create unknown users (default true)
```

- A bearer is first looked up as a mac-mgmt token; only otherwise is it checked at Zitadel's introspection endpoint (results are cached for up to a minute). A token without `project_id` in its audience is rejected (401).
- The user is found by the token's email. An existing user is used as it is (name, organizations and roles unchanged).
- An email without a mac-mgmt user is **created on first use**, exactly as a first web sign-in would create it — provided the token marks the email as verified (`email_verified`; otherwise 403). The web login's rules for the same issuer apply: the `[[auth.providers]]` entry whose `issuer` equals `[zitadel_bearer].issuer` decides who may be created (`allow_all`, `allowed_domains`, `allowed_emails`, `zitadel_org`) and which organizations the new user joins (`auto_join_orgs`, role "read"); `[auth].admin_emails` become admins. An email those rules would refuse at the web login is answered with 403. Without such a provider (e.g. an API-only server), every user the token check admits is created, joining no organization — the token must still carry `project_id` in its audience, so Zitadel's project grants decide who can obtain one. Each creation is written to the audit log with the acting service account.
- `auto_create = false` turns this off: an unknown email is answered with 403, and the user has to sign in to the web UI once (or be invited) first.
- If the token was obtained by impersonation, the acting service account (`act` claim) is recorded: the request runs as `user:<email> via <actor>` and is written to the audit log.
- Without the section only mac-mgmt tokens are accepted.

## Multiple providers

You can configure multiple OIDC providers simultaneously. Each gets its own `[[auth.providers]]` entry with a unique `slug`. The login page shows all configured providers.

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

## Access control

After authentication, mac-mgmt checks the user's email against the provider's access rules:

- **`allowed_domains`** -- the email domain must match one of the listed domains.
- **`allowed_emails`** -- the email must match exactly.
- **`allow_all`** -- any authenticated email is accepted.

These rules are evaluated per-provider based on the JWT's `iss` claim, so different providers can have different access policies.

## Admin emails

Emails listed in `auth.admin_emails` are automatically granted global admin on first login. This is useful for bootstrapping -- after initial setup, admins can be managed from the [User Management](/docs/user-management) page.

## Session storage

Sessions are stored in Redis when `redis_url` is configured, otherwise they fall back to PostgreSQL. Redis is recommended for production deployments with multiple server instances to ensure sessions are shared across replicas.

## Development mode

Set `DEV_ONLY_NO_AUTH=1` to bypass authentication entirely. This uses a hardcoded `dev@localhost` user and should never be used in production.
