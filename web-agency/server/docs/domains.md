---
audience: user
ordering_override: -60
---

# Domain Management

The web agency manages domains across Cloudflare and Spaceship registrars with automatic DNS sync, nameserver configuration, and expiry tracking.

## Adding Domains

### Import from Credential

Go to **Domains** -> **Import from Credential** to bulk-import all domains from a Cloudflare or Spaceship credential. Existing domains are skipped.

### Add Manually

Go to **Domains** -> **Add Domain** to add a single domain. Optionally select a Cloudflare credential to deploy the zone immediately.

### Register a New Domain

Go to **Domains** -> **Register Domain** to check availability and register a new domain via Spaceship.

## Domain Filters

The domain list provides filters to quickly find domains by status:

| Filter | Description |
|--------|-------------|
| **All** | All domains |
| **No CF Zone** | Domains not yet deployed to Cloudflare |
| **Needs NS** | Domains where nameservers don't match the Cloudflare zone |
| **No Webspace** | Domains with a CF zone but no webspace binding |
| **AI Crawl Off** | Domains without AI bot protection |
| **AI Crawl On** | Domains with AI bot protection enabled |
| **SSL Not Full** | Domains with SSL mode not set to Full or Strict |
| **Expires Soon** | Domains expiring within 30 days (shown in red) |

## Bulk Operations

Select multiple domains with the checkboxes, then use the bulk action bar:

- **Deploy to CF** — Deploy selected domains to a Cloudflare zone
- **Set NS at Registrar** — Set Cloudflare nameservers at the Spaceship registrar
- **Set SSL** — Change SSL mode (Off, Flexible, Full, Strict)
- **Set AI Bots** — Enable or disable AI bot protection
- **Create Pages** — Create Cloudflare Pages projects for domains without webspaces

## Domain Detail

Click a domain to view its detail page with:

- DNS records synced from Cloudflare
- Subdomain management
- DNSSEC configuration
- Registration and expiry dates
- Nameserver status

## Automatic Sync

The server periodically syncs (every 6 hours):

- **DNS records** from Cloudflare zones into the local database
- **Expiry dates** from Spaceship and Cloudflare registrars
- **Nameserver status** — checks if registrar NS matches the Cloudflare zone
- **AI bot protection** status from Cloudflare
