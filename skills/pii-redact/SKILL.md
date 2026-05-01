---
name: pii-redact
description: Use when the user wants to share a document with a cloud LLM but it contains PII, company secrets, API keys, or other sensitive information that must not leave the machine.
tools:
  - plan-ai-cleaner
  - plan-ai-cloud
---

# PII Redact & Cloud Query

Share documents with cloud LLMs safely by detecting and redacting sensitive information locally, getting user approval, then sending the cleaned version to the cloud. Responses are automatically rehydrated with original values.

## When to Use

- User wants to analyze a document with a cloud model but it contains sensitive data
- User says "redact", "clean", "remove PII", "strip secrets" before sending to cloud
- User wants to query a cloud LLM with company-internal content
- User needs to share code/logs containing API keys, credentials, or personal data

## Available MCP Tools

### plan-ai-cleaner

| Tool | Purpose |
|------|---------|
| `cleaner_scan` | Scan document text for PII/secrets (regex + local LLM). Returns entity table for review. |
| `cleaner_approve` | Finalize redactions with optional overrides. Returns the redacted document. |
| `cleaner_rehydrate` | Replace placeholders in text with original values from a session. |
| `cleaner_session_list` | List active redaction sessions. |
| `cleaner_session_delete` | Delete a session. |

### plan-ai-cloud

| Tool | Purpose |
|------|---------|
| `cloud_send` | Send a prompt to a cloud LLM. Supports auto-rehydration via `cleaner_session_id`. |
| `cloud_list_providers` | Show configured backends (LiteLLM, Ollama). |
| `cloud_audit_log` | View what was sent to cloud providers. |

## Workflow

### Step 1: Scan the document

```
cleaner_scan(text="<document content>", use_llm=true)
```

This returns a table of detected entities:

```
| Placeholder  | Category | Original        | Source |
|--------------|----------|-----------------|--------|
| [PERSON_1]   | PERSON   | John Smith      | llm    |
| [EMAIL_1]    | EMAIL    | john@acme.com   | regex  |
| [APIKEY_1]   | APIKEY   | sk-proj-abc123  | regex  |
| [COMPANY_1]  | COMPANY  | Acme Corp       | llm    |
```

### Step 2: Get user approval

Present the findings to the user. Ask:
- Are there false positives to remove? (e.g., public company names)
- Anything missing that should be manually added?

### Step 3: Approve with adjustments

```
cleaner_approve(
  session_id="<id>",
  remove_ids=["COMPANY_1"],          // User says Acme Corp is public
  add_redactions=[{"text": "Project Aurora"}]  // User wants this redacted too
)
```

This returns the fully redacted document text.

### Step 4: Send to cloud

```
cloud_send(
  model="anthropic/claude-sonnet-4-6",
  prompt="Analyze the following report for trends: <redacted text>",
  cleaner_session_id="<id>"
)
```

The `cleaner_session_id` parameter triggers automatic rehydration — the cloud response comes back with real names/values restored.

### Step 5 (optional): Manual rehydration

If you sent the redacted text manually or need to rehydrate a different response:

```
cleaner_rehydrate(session_id="<id>", text="<response with placeholders>")
```

## Important Rules

1. **Never send un-scanned text to cloud** — always run `cleaner_scan` first
2. **Always show the entity table to the user** before approving — they decide what's sensitive
3. **Use `cleaner_session_id` on `cloud_send`** for automatic rehydration
4. **If Ollama is unavailable**, use `use_llm=false` for regex-only scanning (still catches emails, API keys, SSNs, etc.)
5. **Sessions expire after 24h** — if a session is expired, re-scan the document

## Entity Categories Detected

**Regex (high confidence):**
- Email addresses, phone numbers (US + international)
- SSNs, credit card numbers
- IPv4/IPv6 addresses (excluding loopback)
- API keys (AWS, GitHub, generic sk-/pk-/token-/secret-)
- JWTs, URLs with embedded credentials

**LLM (contextual):**
- Person names, company/organization names
- Project codenames, internal URLs
- Proprietary terminology

## Model Routing

- Cloud models (e.g., `anthropic/claude-sonnet-4-6`, `openai/gpt-5.4`): routed through LiteLLM proxy
- Local models (prefix `ollama/`): sent directly to Ollama, bypassing LiteLLM
- Default: uses LiteLLM's configured default model

## Error Handling

- If `cleaner_scan` fails to reach Ollama: falls back to regex-only detection, warn user
- If `cloud_send` fails: show the error, suggest checking LiteLLM status or API keys
- If session is expired: re-scan the document (sessions last 24h by default)
