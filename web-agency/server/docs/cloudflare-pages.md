---
audience: user
ordering_override: -40
---

# Cloudflare Pages Webspaces

Cloudflare Pages webspaces host static sites and JAMstack applications on Cloudflare's global CDN.

## Deployment Sources

When creating a Pages project, you must choose between **Git** and **Direct Upload**. This choice is permanent and cannot be changed after creation.

### Git Repository

Connects a GitHub or GitLab repository. Cloudflare automatically builds and deploys on every push.

**Requirements:**
- The GitHub/GitLab integration must be [authorized in your Cloudflare dashboard](https://dash.cloudflare.com/?to=/:account/pages)
- The API token needs **Cloudflare Pages: Edit** permission

**Configuration:**
- Provider (GitHub or GitLab)
- Owner (user or organization)
- Repository name
- Production branch (e.g. `main`)
- Build command (e.g. `npm run build`)
- Output directory (e.g. `dist`)

### Direct Upload

Upload site files as a tarball via the deploy API. Best for CI/CD pipelines or custom build systems.

See [Deploying with Tarballs](/docs/deploying-with-tarballs) for complete instructions.

## Custom Domains

Bind domains or subdomains to a Pages webspace on the webspace detail page. For each binding:

1. The domain is added as a custom domain on the Cloudflare Pages project
2. Cloudflare automatically provisions an SSL certificate
3. The site becomes accessible at the custom domain

## Preview URLs

Every Pages project gets a `*.pages.dev` subdomain for preview deployments, shown on the webspace detail page.
