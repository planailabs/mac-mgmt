---
audience: admin
ordering_override: -33
---

# Basic Auth Internals

Developer reference for the cookie-based basic-auth gate that replaced the HTTP
Basic challenge. It spans three components — the **proxy** (`web-agency/proxy`),
the agency **server** (`web-agency/server`), and the shared **plan-ai-html**
crate — plus three database tables. This page explains each piece and how they
interlock.

## Why a dance at all

The webspace domain (e.g. `site.example.com`) and the agency domain are
different origins. The gate is enforced by the proxy on the webspace domain, so
the session cookie must live there — but the login form lives on the agency
domain. Crossing that boundary requires a signed token in the URL (the same
shape as the existing OIDC `/proxy-gate` flow). This is the Cloudflare-Access
pattern, hence the `cgi-webagency/` path naming.

## Identity model (DB)

Migration `027_basic_auth_sessions.sql` adds three tables:

| Table | Role |
|-------|------|
| `basic_auth_sso` | A browser **identity** (`id`, `expires_at`). |
| `basic_auth_sso_lists` | Which lists an identity has proven, and as whom (`sso_id`, `list_id`, `username`). |
| `basic_auth_sessions` | One row per **cookie** (agency cookie + each webspace-domain cookie): `sso_id`, `token_hash` = sha256(opaque token), `expires_at`. |

List authorizations hang off the **identity**, not the individual cookie. That's
what makes one sign-in cover every site sharing a list, and what lets per-list
sign-out work. `ON DELETE CASCADE` from `basic_auth_sso` wipes an identity's
cookies and list grants together.

## Cookies

| Cookie | Domain | Set/cleared by | Purpose |
|--------|--------|----------------|---------|
| `__basic_session` | webspace | proxy | The gate cookie the proxy verifies per request. |
| `__agency_basic_sso` | agency | agency server | The SSO cookie that enables silent re-auth on other sites. |

Both are `HttpOnly; Secure; SameSite=Lax`, opaque random tokens; only their
sha256 is stored. `Max-Age` is one year when "keep me signed in" is ticked,
otherwise a session cookie (DB rows still carry a 12h / 1y backstop expiry).

## Signed handoff token

`agency → proxy` after a successful login. Format:
`base64url(json) + "." + hex(hmac_sha256(internal_token, base64url(json)))`,
payload `{sso, list, u, keep, exp}` with `exp ≈ now+120s`. The shared key is the
proxy `internal_token` both sides already hold. The agency mints it; the server
(`/api/internal/basic/session`) verifies it authoritatively.

## Internal API (server, Bearer `internal_token`)

Defined in `api::basic_auth::internal_router`, merged into the internal router.

| Endpoint | Body | Returns | Notes |
|----------|------|---------|-------|
| `POST /api/internal/basic/verify` | `{session_token, list_id}` | `{valid, username?, list_name?}` | Joins sessions→sso→sso_lists→lists; checks both expiries and the list grant. |
| `POST /api/internal/basic/session` | `{handoff_token}` | `{session_token, max_age_secs?}` | Verifies the handoff, creates a `basic_auth_sessions` row for `sso`. |
| `POST /api/internal/basic/logout` | `{session_token, list_id}` | `204` | Deletes the one `sso_lists` row (per-list sign-out); prunes empty identities. |

The routes feed (`/api/internal/routes`) now ships `auth: {mode:"basic",
basic_list_id}` instead of `basic_credentials` — the proxy never receives
password hashes.

## Agency pages (server, public — NOT behind OIDC `require_auth`)

`api::basic_auth::agency_router`, merged separately from the OIDC-gated
`web_router`. Rendered with **plan-ai-html** (mustache + plan-ai-design styling).

| Route | Purpose |
|-------|---------|
| `GET /agency/basic/{list}/login` | Login form. If the SSO cookie already proves this list → mint handoff and redirect silently (no form). |
| `POST /agency/basic/{list}/login` | Validate credentials, upsert the identity + list grant, set the SSO cookie, mint a handoff, redirect to the webspace callback. |
| `GET /agency/basic/{list}/logout` | Informational "signed out" page. |
| `GET /agency/basic/{list}/profile` | Sign-in status for one list + other lists on the identity. |
| `GET /agency/basic/profile` | Session manager: every list the identity holds, with per-list and "sign out everywhere" actions. |
| `POST /agency/basic/profile/logout` | `{list_id}` drops one grant; `{all=1}` deletes the whole identity and clears the SSO cookie. |

Open-redirect defense: the `cb` callback host is validated against the domains
actually bound to webspaces using that list; the proxy additionally requires
`back` to be same-origin with the webspace host.

## Proxy (per request)

`AuthMode::Basic { list_id }`. In `request_filter`, for a Basic folder the proxy
strips the mount prefix and inspects the sub-path:

- `…/cgi-webagency/basic` → JSON whoami (calls `verify`).
- `…/cgi-webagency/basic/{login,logout,profile}` → the control handlers
  (`handle_basic_cgi`), driving the redirect dance and setting/`__basic_session`.
- anything else → **gate**: read `__basic_session`, call `verify`; pass through
  if valid, else redirect to `…/cgi-webagency/basic/login?back=<url>`.

Verification is a per-request call to the internal API, so sign-out is immediate.
If the server is unreachable the gate **fails closed**.

## End-to-end flows

**Login:** gated hit → 302 `…/cgi-webagency/basic/login?back` → 302
`/agency/basic/{list}/login?back&cb` → form → POST validates → 302
`{cb}/login?token&back` → proxy calls `/session`, sets cookie → 302 `back`.

**Silent SSO:** on a second site sharing the list, the agency login GET sees the
SSO cookie already proves the list and skips straight to the handoff.

**Logout (per-list):** `…/cgi-webagency/basic/logout` → proxy calls `/logout`
(drops the one `sso_lists` row) → bounces to the agency confirmation page. Other
lists keep working; the global manager can drop all.

## plan-ai-html

`plan-ai-html` is a standalone crate that renders self-contained HTML pages
styled with the plan-ai-design tokens/classes (inlined, no external stylesheet)
via mustache. It's shared by the basic-auth pages, the OIDC provider login page
(`plan-ai-auth`), and the relay "authentication required" page so they all match.
