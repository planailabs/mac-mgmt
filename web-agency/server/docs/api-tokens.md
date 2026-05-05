---
audience: admin
ordering_override: -20
---

# API Tokens

API tokens provide programmatic access to the web-agency server.

## Token Kinds

### API Tokens

General-purpose tokens for the REST API. Optionally scoped to an organization.

### Deploy Tokens

Used to upload tarballs to Cloudflare Pages webspaces via the deploy API. Can be scoped to a specific webspace or left as "All webspaces".

**Deploy API endpoints:**
- `POST /api/v1/deploy/:webspace_id` — upload a tarball
- `GET /api/v1/deploy/:webspace_id/status` — check deployment status

See [Deploying with Tarballs](/docs/deploying-with-tarballs) for usage details.

## Creating Tokens

1. Go to **Tokens** → **Create Token**
2. Enter a descriptive label
3. Select the token kind (API or Deploy)
4. For deploy tokens, optionally scope to a specific webspace
5. Copy the token immediately — it's shown only once

## Security

- Tokens are stored as SHA-256 hashes — the plaintext is never stored
- Tokens can be revoked from the token list
- Deploy tokens are validated against the webspace_id in the URL
- Use separate tokens per CI/CD pipeline for easy revocation
