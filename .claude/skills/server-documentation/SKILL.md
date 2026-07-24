---
name: server-documentation
description: Write and update server web UI documentation. Reads all server components, checks admin gating, and maintains bilingual (English + German) docs in server/docs/ with audience frontmatter badges.
---

# Server Documentation Skill

You are updating the mac-mgmt server documentation. Documentation lives in `server/docs/` as markdown files rendered by the web UI at `/docs`.

**The docs are bilingual.** English files in `server/docs/*.md` are canonical; German translations live in `server/docs/de/` with identical filenames. Every doc change updates both languages in the same commit (see "Step 5b").

## Step 1: Read existing documentation

Read every `.md` file in `server/docs/` to understand what is already documented, the writing style, and the frontmatter structure.

## Step 2: Read all server web components

Read every file in `server/src/web/components/`. For each component, note:
- What feature/page it implements
- Whether the server function calls `require_admin()` (admin-only) or is available to all authenticated users
- What data it displays or modifies

Also check `server/src/web/components/navbar.rs` to see which nav links are in `common_links` (all users) vs `admin_links` (admin only).

## Step 3: Check API routes for auth gating

Read `server/src/api/routes.rs` and `server/src/api/auth.rs` to understand:
- Which API endpoints use `SyncAuth` (daemon sync tokens)
- Which use `SettingAuth` (setting tokens, available to non-admin users with tokens)
- Which use `AdminAuth` (admin-only)

## Step 4: Check configuration schema

Read `common/src/lib.rs` for the `ClusterConfig` struct and all nested config types. This is the source of truth for configuration fields, defaults, and descriptions.

## Step 5: Write or update documentation

For each feature area, create or update a `.md` file in `server/docs/`.

### Frontmatter format

Every doc file MUST have YAML frontmatter with an `audience` field:

```markdown
---
audience: user
---

# Page Title

Content here...
```

Valid audience values:
- `user` — features accessible to all authenticated users (no `require_admin()` gate)
- `admin` — features that require admin access (`require_admin()` in the component's server functions)

### Audience assignment rules

- If ALL server functions in a component call `require_admin()`, the doc is `admin`
- If the component is accessible to all authenticated users, the doc is `user`
- If a topic covers both admin and user features, split it into separate docs (e.g. `api-usage.md` for users, `tokens-and-api.md` for admin token management)
- Check the navbar: features in `common_links` are user-facing, features in `admin_links` are admin-facing

### Writing style

- Use backticks for config parameters (`global.llm_provider`), URLs, API endpoints, and code
- Use tables for structured reference data (config fields, API endpoints)
- Use ## and ### for section hierarchy, # only for the page title
- Keep descriptions concise and factual
- Include default values for config fields
- Cross-link related docs using markdown links to `/docs/<slug>`
- Do not repeat information already covered in another doc; link to it instead

### What to document

For each feature area, cover:
- What the feature does and why you'd use it
- How to use it (step-by-step if applicable)
- Configuration fields with defaults and valid values
- API endpoints if relevant (method, path, description)
- Any precedence rules or non-obvious behavior

### What NOT to document

- Internal implementation details (database schema, Rust types)
- Code examples in Rust
- Anything derivable from the Swagger UI

## Step 5b: Keep the German translation in sync

For every English doc you create or change, create/update `server/docs/de/<same-filename>` in the same commit:

- Frontmatter stays byte-identical (`audience` values are English keywords; `ordering_override` unchanged).
- Translate headings (including the `# Title` — it is the displayed title), prose, and table header/description columns. Formal address ("Sie").
- Never translate: slugs, cross-link targets (`/docs/<slug>`), config keys, code blocks, API endpoints, CLI commands, defaults.
- Use the German UI terminology from `server/src/web/de-DE.ftl` for bolded UI labels (e.g. **Users** → **Benutzer**) so docs match the rendered UI. Product terms (Cluster, Daemon, Skill, Token, Rollout, Healer, Relay, Heartbeat) stay as loanwords, matching the UI.

## Step 5c: Screenshots for step-by-step guides

Step-by-step guides embed language-specific screenshots from `server/public/docs-img/`, referenced as `![...](/docs-img/<name>-en.png)` in English docs and `.../<name>-de.png` in German docs. They carry numbered orange step markers matching the numbered steps in the doc text.

Regenerate them with `server/scripts/regen-docs-screenshots.mjs` (header comment documents prereqs: dev app with `DEV_ONLY_NO_AUTH=1` on a scratch database, Chrome on PATH). When you add a new guide flow, extend the script's `capture()` with the new page(s) and markers — element lookup must use the per-language labels in its `LANGS` table. Regenerate whenever UI changes make existing screenshots stale.

## Step 6: Verify

After writing, verify:
- Every doc has valid frontmatter with `audience: user` or `audience: admin`
- No admin-gated features are documented as `audience: user`
- No user-accessible features are documented as `audience: admin`
- Cross-links between docs use correct slugs (filename without `.md`)
- The project compiles: `cargo check -p mac-mgmt-server`
