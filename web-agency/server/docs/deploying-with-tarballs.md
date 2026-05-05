---
audience: developer
ordering_override: -50
---

# Deploying with Tarballs

Deploy your site to Cloudflare Pages by uploading a tarball through the web-agency API. This is the recommended method for CI/CD pipelines.

## Prerequisites

- A webspace with a **Direct Upload** Cloudflare Pages project
- A **deploy token** (create one at Tokens → Create Token → Kind: Deploy)
- `curl` and `tar` installed

## Step 1: Create a Deploy Token

1. Go to **Tokens** → **Create Token**
2. Set **Token Kind** to **Deploy**
3. Optionally scope it to a specific webspace under **Webspace Scope**
4. Click **Create Token** and copy the token (it's shown only once)

## Step 2: Build Your Site

Build your static site as usual. The output should be a directory (e.g. `./dist`, `./build`, `./out`).

```bash
# Example: Next.js
npm run build

# Example: Vite
npx vite build

# Example: Hugo
hugo --minify
```

## Step 3: Create a Tarball

Package the build output into a `.tar.gz` file:

```bash
tar czf site.tar.gz -C ./dist .
```

> **Important:** Use `-C ./dist .` (not `./dist`) so files are at the root of the archive, not nested in a `dist/` subdirectory.

## Step 4: Upload

```bash
curl -X POST \
  -H "Authorization: Bearer YOUR_DEPLOY_TOKEN" \
  --data-binary @site.tar.gz \
  https://your-server.example.com/api/v1/deploy/WEBSPACE_ID
```

Replace:
- `YOUR_DEPLOY_TOKEN` with the token from Step 1
- `WEBSPACE_ID` with your webspace's UUID (visible in the URL on the webspace detail page)

The response includes a `deployment_id` and initial status:

```json
{
  "deployment_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "uploading"
}
```

## Step 5: Check Status

Poll the status endpoint:

```bash
curl -H "Authorization: Bearer YOUR_DEPLOY_TOKEN" \
  https://your-server.example.com/api/v1/deploy/WEBSPACE_ID/status
```

Response:

```json
{
  "deployment_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "success",
  "error_message": null,
  "tarball_size": 1048576,
  "created_at": "2025-01-15T10:30:00Z",
  "updated_at": "2025-01-15T10:31:00Z"
}
```

### Status Values

| Status | Meaning |
|--------|---------|
| `pending` | Deployment created, not yet started |
| `uploading` | Tarball received, being written to disk |
| `deploying` | Tarball extracted, `wrangler pages deploy` running |
| `success` | Deployment completed successfully |
| `failed` | Deployment failed (check `error_message`) |

## CI/CD Example: GitHub Actions

```yaml
name: Deploy
on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build
        run: npm ci && npm run build

      - name: Package
        run: tar czf site.tar.gz -C dist .

      - name: Deploy
        run: |
          RESPONSE=$(curl -s -X POST \
            -H "Authorization: Bearer ${{ secrets.DEPLOY_TOKEN }}" \
            --data-binary @site.tar.gz \
            ${{ vars.SERVER_URL }}/api/v1/deploy/${{ vars.WEBSPACE_ID }})
          echo "$RESPONSE"
          STATUS=$(echo "$RESPONSE" | jq -r '.status')
          if [ "$STATUS" = "failed" ]; then exit 1; fi

      - name: Wait for deploy
        run: |
          for i in $(seq 1 30); do
            sleep 5
            STATUS=$(curl -s \
              -H "Authorization: Bearer ${{ secrets.DEPLOY_TOKEN }}" \
              ${{ vars.SERVER_URL }}/api/v1/deploy/${{ vars.WEBSPACE_ID }}/status \
              | jq -r '.status')
            echo "Status: $STATUS"
            if [ "$STATUS" = "success" ]; then exit 0; fi
            if [ "$STATUS" = "failed" ]; then exit 1; fi
          done
          echo "Timeout waiting for deploy"
          exit 1
```

## Troubleshooting

### "token kind must be 'deploy'"
You're using a regular API token. Create a new token with Kind set to **Deploy**.

### "token not scoped to this webspace"
The deploy token is scoped to a different webspace. Create a new token scoped to this webspace, or use a token scoped to "All webspaces".

### "Pages project not deployed yet"
The webspace needs a Cloudflare Pages project first. Go to the webspace detail page and create a Pages project with **Direct Upload** as the deployment source.

### Deployment stuck in "deploying"
The wrangler process may have hung. Check server logs. Ensure `npx` and `wrangler` are installed on the server. The server runs `npx wrangler pages deploy` with `CLOUDFLARE_API_TOKEN` set from the linked credential.

### "wrangler failed" error
The Cloudflare API token in the linked credential may lack **Cloudflare Pages: Edit** permission. Update the token in the Cloudflare dashboard.
