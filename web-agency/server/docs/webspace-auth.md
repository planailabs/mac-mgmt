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
| **HTTP Basic** | A styled sign-in page validating against a reusable credential list, with a cookie session shared across sites that use the same list. |

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

Basic auth no longer uses the browser's built-in username/password popup. Instead, visitors get a normal, styled sign-in page and a cookie session.

### Signing in

1. Visit a Basic-protected area. If you're not signed in, you're redirected to a **Sign in** page.
2. Enter a username and password from the area's [Basic Auth List](/docs/basic-auth-lists).
3. Optionally tick **Keep me signed in** to stay signed in for a year (otherwise the session ends when you close the browser).
4. You're sent back to the page you were trying to reach, now signed in.

Because the session is tied to your identity (not a single site), **signing in once covers every site that uses the same list** — the next such site signs you in silently, without showing the form again.

### Signing out

- **One area:** visit `…/<folder>/cgi-webagency/basic/logout`. This signs you out of that list only; other lists stay signed in.
- **Everything:** open the session manager at `https://<agency-domain>/agency/basic/profile` to see every area you're signed in to and sign out of one or all of them.

### Control paths

Each Basic-protected folder exposes these paths (relative to the folder):

| Path | Purpose |
|------|---------|
| `cgi-webagency/basic/login` | Start the sign-in flow (`?back=` sets where to return) |
| `cgi-webagency/basic/logout` | Sign out of this area's list |
| `cgi-webagency/basic/profile` | Show sign-in status for this area |
| `cgi-webagency/basic` | JSON `{username, list_id, list_name}` when signed in, `401` otherwise — handy for client-side checks |

The session is verified on every request, so signing out takes effect immediately. Credentials are validated only at sign-in (the proxy never receives them); passwords are stored hashed with SHA-256.

For how this works under the hood, see [Basic Auth Internals](/docs/basic-auth-internals).
