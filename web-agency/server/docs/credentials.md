---
audience: admin
ordering_override: -30
---

# Managing Credentials

Credentials store API tokens for Cloudflare and Spaceship registrar integrations. They are encrypted at rest using AES-256-GCM.

## Credential Types

### Cloudflare

JSON format:
```json
{
  "api_token": "your-cloudflare-api-token",
  "account_id": "your-cloudflare-account-id"
}
```

**Required API Token Permissions:**
- Zone: Zone → Read (for listing/managing zones)
- Zone: DNS → Edit (for DNS record management)
- Account: Cloudflare Pages → Edit (for Pages project management)

**Finding your Account ID:**
Log into the Cloudflare dashboard → select any zone → the account ID is in the right sidebar under "API".

### Spaceship

JSON format:
```json
{
  "api_key": "your-spaceship-api-key",
  "api_secret": "your-spaceship-api-secret"
}
```

Generate keys at [Spaceship API Manager](https://www.spaceship.com/application/api-manager/).

## Organization Assignment

Credentials can optionally be assigned to an organization. Global credentials (no org) are available to all users.

## Security

- Credential data is encrypted with AES-256-GCM before storage
- The encryption key is configured in `config.toml` under `[secrets].encryption_key`
- Credentials are only decrypted at the moment they're used for API calls
- The plaintext credential data is never sent to the browser
