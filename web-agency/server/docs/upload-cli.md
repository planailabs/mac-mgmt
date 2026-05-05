---
audience: developer
ordering_override: -45
---

# Upload CLI (web-agency-upload)

A tiny CLI tool for deploying site folders to Cloudflare Pages webspaces. It tars the folder, uploads it to the deploy API, and waits for the deployment to complete.

## Installation

```bash
cargo install --path web-agency/upload-cli
```

## Usage

```bash
# Minimal (token and URL from env, webspace from token scope)
export WEB_AGENCY_TOKEN="your-deploy-token"
export WEB_AGENCY_URL="https://your-server.example.com"
web-agency-upload ./dist

# Explicit arguments
web-agency-upload ./dist \
  --token YOUR_TOKEN \
  --url https://your-server.example.com \
  --webspace-id 550e8400-e29b-41d4-a716-446655440000

# Don't wait for deploy to finish
web-agency-upload ./dist --no-wait
```

## Arguments

| Argument | Env Variable | Required | Description |
|----------|-------------|----------|-------------|
| `PATH` | — | Yes | Directory to upload |
| `--token` | `WEB_AGENCY_TOKEN` | Yes | Deploy token |
| `--url` | `WEB_AGENCY_URL` | Yes | Server URL |
| `--webspace-id` | `WEB_AGENCY_WEBSPACE_ID` | No | Webspace UUID. Auto-detected from token scope if omitted. |
| `--poll-interval` | — | No | Seconds between status polls (default: 3) |
| `--no-wait` | — | No | Exit after upload without waiting |

## Auto-detecting Webspace

If `--webspace-id` is not specified, the CLI calls `GET /api/v1/deploy/whoami` to check the token's scope. If the token is scoped to a single webspace, that webspace is used automatically.

For tokens scoped to "All webspaces", you must specify `--webspace-id`.

## Example Output

```
$ web-agency-upload ./dist
No --webspace-id specified, checking token scope...
Using token's scoped webspace: my-site (550e8400-...)
Packaging ./dist...
Tarball size: 45632 bytes (44.6 KB)
Uploading to webspace 550e8400-...
Deployment abc123 started (status: uploading)
Waiting for deployment...
Status: deploying
Status: success
Deployment successful!
{"deployment_id":"abc123","status":"success"}
```

## CI/CD Integration

### GitHub Actions

```yaml
- name: Deploy
  env:
    WEB_AGENCY_TOKEN: ${{ secrets.DEPLOY_TOKEN }}
    WEB_AGENCY_URL: ${{ vars.SERVER_URL }}
  run: web-agency-upload ./dist
```

### GitLab CI

```yaml
deploy:
  script:
    - web-agency-upload ./dist
  variables:
    WEB_AGENCY_TOKEN: $DEPLOY_TOKEN
    WEB_AGENCY_URL: $SERVER_URL
```
