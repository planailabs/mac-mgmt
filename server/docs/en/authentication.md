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
