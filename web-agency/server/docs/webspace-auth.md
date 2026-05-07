---
audience: user
ordering_override: -35
---

# Webspace Authentication

Non-Pages webspaces (Local, Relay Tunnel, Tunnel) can require authentication before serving content. Auth is enforced by the reverse proxy before forwarding requests to the upstream.

## Auth Modes

| Mode | Description |
|------|-------------|
| **None** | No authentication (default) |
| **OIDC** | Users must log in via the agency's OIDC provider. Only members of the webspace's organization (any role) or admins can access. |
| **HTTP Basic** | Standard HTTP Basic authentication against a reusable credential list. |

Pages webspaces are served through Cloudflare and cannot use proxy-level auth.

## Configuring Auth

On the webspace detail page, the **Auth** section lets you change the auth mode:

1. Select **OIDC** or **HTTP Basic** from the dropdown
2. For HTTP Basic, select a [Basic Auth List](/docs/basic-auth-lists) containing the allowed credentials
3. Click **Save**

Changes take effect on the next proxy route reload (typically within seconds via SSE).

## OIDC Flow

When a user visits an OIDC-protected webspace:

1. The proxy redirects to the agency login page
2. The user authenticates via the configured OIDC provider (if not already logged in)
3. The agency verifies the user is a member of the webspace's organization
4. A signed session cookie is set on the webspace domain (valid 24 hours)
5. Subsequent requests are served without re-authentication until the cookie expires

The session cookie is domain-scoped — access to one webspace does not grant access to others.

## HTTP Basic Flow

When a user visits a Basic-protected webspace:

1. The proxy returns `401 Unauthorized` with a `WWW-Authenticate: Basic` header
2. The browser prompts for username and password
3. Credentials are validated against the selected [Basic Auth List](/docs/basic-auth-lists)
4. If valid, the request is forwarded to the upstream

Credentials are checked on every request (stateless). The password is hashed with SHA-256 for comparison.
