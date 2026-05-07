---
audience: user
ordering_override: -100
---

# Getting Started

Welcome to the Web Agency server — a management dashboard for domains, DNS, Cloudflare Pages webspaces, and billing.

## Overview

The Web Agency server lets you:

- **Manage domains** across Cloudflare and Spaceship registrars
- **Import existing domains** from your Cloudflare or Spaceship accounts
- **Provision DNS** with automatic Cloudflare zone management
- **Host websites** via Cloudflare Pages (git or direct upload) or local nginx
- **Track billing** for domain registration, renewal, and hosting costs
- **Manage credentials** for Cloudflare and Spaceship APIs

## First Steps

### 1. Add a Credential

Go to **Credentials** → **Add Credential** and configure either:

- **Cloudflare**: API token + account ID (from the Cloudflare dashboard)
- **Spaceship**: API key + API secret (from the Spaceship API Manager)

### 2. Import or Add Domains

- **Import**: Go to **Domains** → **Import from Credential** to bulk-import all domains from a credential
- **Add manually**: Go to **Domains** → **Add Domain** to add a single domain and optionally deploy it to Cloudflare

### 3. Deploy to Cloudflare

On a domain's detail page, use the **Cloudflare** section to:

1. Select a credential and deploy the zone
2. Nameservers are automatically set at the registrar (if Spaceship)
3. Configure SSL mode and DNSSEC

### 4. Create a Webspace

Go to **Webspaces** → **Create Webspace** and choose:

- **[Cloudflare Pages](/docs/cloudflare-pages)**: Deploy via git repo or direct upload (tarballs)
- **Local**: Host on the server with Pingora + ACME SSL
- **[Relay Tunnel](/docs/relay-tunnel-webspaces)**: Proxy through a mac-mgmt relay server
- **[Tunnel](/docs/relay-tunnel-webspaces)**: Plain reverse proxy to any upstream URL

### 5. Bind Domains to Webspaces

On the webspace detail page, bind domains or subdomains. For Cloudflare Pages, custom domains are automatically configured. For Relay/Tunnel webspaces, a CNAME record is created pointing to the agency domain.

### 6. (Optional) Protect with Auth

On the webspace detail page, set an [auth mode](/docs/webspace-auth):

- **OIDC**: Require login via the agency's OIDC provider (org members only)
- **HTTP Basic**: Require credentials from a [Basic Auth List](/docs/basic-auth-lists)

## Concepts

### Organizations

Resources (domains, webspaces) belong to organizations. Users are members of organizations with roles:

| Role | Permissions |
|------|-------------|
| **read** | View domains, webspaces, billing |
| **write** | Manage domains, webspaces, DNS records |
| **admin** | Manage members, create tokens |

### Credentials

API credentials for Cloudflare and Spaceship. Encrypted at rest (AES-256-GCM). Optionally assigned to an organization, or global.

### Subdomains

Subdomains are entities under a domain (e.g. `www`, `@`, `api`). Each subdomain can have multiple DNS records and can be individually bound to a webspace.
