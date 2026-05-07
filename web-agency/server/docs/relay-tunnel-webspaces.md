---
audience: user
ordering_override: -38
---

# Relay & Tunnel Webspaces

In addition to [Cloudflare Pages](/docs/cloudflare-pages), the web agency can proxy traffic to external upstreams via **Relay Tunnel** and **Tunnel** webspaces. Both are served through the built-in Pingora reverse proxy with automatic ACME TLS certificates.

## Hosting Types

### Relay Tunnel

Proxies traffic through a [mac-mgmt relay](https://git.plan.ai/plan-ai/mac-mgmt) server. The proxy authenticates with the relay using a short-lived token minted from a mac-mgmt credential.

**Required fields:**
- **Relay URL**: Full URL of the relay endpoint (e.g. `https://abc123-ollama.relay.plan.ai`)
- **mac-mgmt Credential**: A mac-mgmt credential with a server URL and admin token, used to mint proxy tokens

### Tunnel (Plain Reverse Proxy)

Proxies traffic directly to any upstream URL without relay authentication. Use this for services with a stable public or internal URL.

**Required fields:**
- **Upstream URL**: URL to reverse-proxy to (e.g. `https://backend.example.com`)

## Creating a Relay or Tunnel Webspace

1. Go to **Webspaces** -> **Create Webspace**
2. Select the organization
3. Choose **Relay Tunnel** or **Tunnel** as the hosting type
4. Enter the upstream/relay URL
5. For Relay: select a mac-mgmt credential
6. Optionally set an [auth mode](/docs/webspace-auth) (None, OIDC, or HTTP Basic)
7. Click **Create**

## Binding Domains

On the webspace detail page, bind domains or subdomains in the **Domain Bindings** section. When binding a domain:

- A CNAME record pointing to the agency domain is automatically created in the domain's Cloudflare zone
- The proxy automatically requests an ACME TLS certificate for the hostname
- The **CNAME** column shows whether the DNS record is properly configured, with a **Fix** button if missing

## Editing Settings

The **Settings** section on the webspace detail page lets you change:

- **Name**: Display name for the webspace
- **Upstream URL**: The target URL (relay or tunnel)

## Deleting a Webspace

The **Danger Zone** section at the bottom of the detail page lets you delete the webspace. All domain bindings and deployments will be removed.
