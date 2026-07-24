# Build & Test

- `cargo check --workspace` to verify all crates compile.
- `cargo test -p <crate>` to run tests for a specific crate.

## Web UI (server crate)

When modifying files under `server/src/web/` or anything that affects the Dioxus frontend (components, shared types used by the web UI in `common/src/lib.rs`), run:

```
cd server && dx build
```

This builds both the server-side (native) and the client-side (WASM) targets. The WASM build catches issues invisible to `cargo check` alone (e.g. dependencies that don't support `wasm32-unknown-unknown`).

**Feature gate rule:** Dependencies that don't compile to WASM (sentry, hostname, uname, etc.) must be behind the `server` or `server-api-only` feature flags so they're excluded from the WASM client build.

## Daemon: verify builds across feature combinations

When changing daemon code, ensure it compiles not only with default features but also with these additional feature combinations:

- `--no-default-features` (no features at all)
- `--no-default-features --features self-update` (only self-update)
- `--all-features` (all features enabled)
- `--no-default-features --features services,relay,self-update` (non-default features enabled alongside defaults)

Run `cargo check -p mac-mgmt` with each combination to catch gating issues early. Code behind a feature gate must not reference items from another feature without the appropriate `#[cfg(feature = "...")]` guard.

## Server: verify builds across feature combinations

The server features are `server`, `webui`, `web`, and `server-api-only`. When changing server code, ensure it compiles in these modes:

- `cargo check -p mac-mgmt-server` (default — `server`, `webui`, `web` all on)
- `cargo check -p mac-mgmt-server --no-default-features --features "server-api-only"` (API-only, no web UI)

When adding catalog mutation paths (skills, bundles, MCP servers, MCP bundles), call `crate::api::push::notify_federation_global()` so federation SSE subscribers are notified of catalog changes.

## Configuration

When changing default values in config structs (`OllamaConfig`, `OpenClawConfig`, etc.), always update `config.example.toml` to reflect the new defaults.

## CI/build performance

Do not add `CARGO_BUILD_JOBS` restrictions to Nix or Dioxus builds. They make CI builds unnecessarily slow and are not needed for the current builders; fix the underlying build issue instead of throttling Cargo globally.

## Daemon: Running external commands

Always use `crate::cmd::output_with_timeout()` instead of `Command::new(...).output()` for any external command executed during the daemon's event loop (health checks, connectors, repair). Bare `.output()` has no timeout and can hang the daemon indefinitely. Use `cmd::DEFAULT_TIMEOUT` (30s) unless the command is known to be slow (e.g. `openclaw doctor --fix` uses 60s).

## mac-mgmt-services: protocol changes must be two-way backwards compatible

The supervisor and the daemon are upgraded independently (the supervisor keeps running across daemon self-updates, and a new daemon will commonly talk to an older supervisor and vice-versa). Any change to `mac-mgmt-services/src/protocol.rs` must stay compatible in both directions:

- New fields on existing variants go behind `#[serde(default)]` (readers that don't know them ignore them; readers that expect them tolerate their absence).
- When you replace a field, keep the old one on the wire during the transition and have the new side fall back to it when the new one is empty/missing (see the `statuses` / `names` pair on `Response::Services`).
- Don't rename or remove existing request/response variants; add new ones and keep handling the old shape.
- When in doubt, test with one side on the old protocol and one on the new.

Mark every compat shim with a `// compat: added YYYY-MM-DD, removable after YYYY-MM-DD` comment (three months out) so a later cleanup pass can delete shims confidently instead of guessing whether something out in the wild still needs them.

## Server: database migrations are append-only

Never modify an existing migration file in `server/migrations/`. Migrations that have already been applied to a database cannot be re-run, so editing them has no effect on deployed instances and causes checksum mismatches. Always create a new migration with the next sequence number instead (e.g. if `033_*.sql` exists, create `034_*.sql`).

## Server docs: keep English and German in sync

Server web UI documentation lives in `server/docs/*.md` (English, canonical) with German translations under `server/docs/de/` using **identical filenames/slugs**. Whenever you add or change a doc, update **both languages in the same commit** — never let them drift:

- New doc → write the English file and its German translation together.
- Edited doc → apply the same content change to the other language.
- Frontmatter (`audience`, `ordering_override`) is only read from the English file, but keep it byte-identical in the German file anyway.
- Keep slugs, cross-links (`/docs/<slug>`), config keys, code blocks, and API endpoints untranslated; use the German UI terminology from `server/src/web/de-DE.ftl` for UI labels. Formal address ("Sie").
- Screenshots in docs are language-specific (`/docs-img/<name>-en.png` / `-de.png`). Regenerate them with `server/scripts/regen-docs-screenshots.mjs` (see the header comment for prereqs) after UI changes that make them stale, and reference the `-en` variant from English docs and `-de` from German docs.

## Config migrations: always add one when modifying config properties

When changing config struct fields in `common/src/lib.rs` (renames, type changes, structural changes like turning a single field into a list), you **must** add a corresponding config migration in `common/src/config_migrate.rs`. This ensures existing JSON configs stored in the database and served to daemons are transformed automatically.

Steps:

1. Write an idempotent migration function `fn migrate_NNN_description(config: &mut Value)` in `common/src/config_migrate.rs`. It must detect whether the old format is present and transform it — **never** assume the input needs migration.
2. Append a call to the new function at the end of the `migrate()` function.
3. Add tests covering both "needs migration" and "already migrated" (idempotent) cases, plus a test that the migrated JSON parses as `ClusterConfig`.
4. Add a SQL migration in `server/migrations/` to transform existing `cluster_configs` rows in the database.
5. When renaming fields, also add `#[serde(alias = "old_name")]` to the struct field so local TOML configs with the old name continue to parse via serde.

The daemon applies `config_migrate::migrate()` to remote JSON configs before deserializing. If deserialization still fails after migration, the daemon falls back to local config. The server applies migrations when reading configs from the database (GET /api/config, web UI) and before validating incoming configs (PUT, PATCH).

## Web-agency: regenerating OpenAPI progenitor crates

When fixing spec-vs-reality mismatches or updating API clients, add the fix to `web-agency/scripts/regenerate-api-crates.py` (e.g. in `fix_cd_spec_issues`, `post_gen_fixups`, or a new trim function). Then always run the **full** script to regenerate all crates:

```
nix-shell -p python3 python3Packages.requests yq-go --run "python3 web-agency/scripts/regenerate-api-crates.py"
```

Do not run partial Python snippets to regenerate individual crates — the full script ensures consistent spec downloads, trimming, and generation across all API crates. Partial runs are only acceptable for quickly testing a new fixup before integrating it into the script.

## Local development: checking logs

When debugging locally (Procfile-based or manual runs), runtime logs are written to:

- `/tmp/mac-mgmt-relay.log` — relay (p2p, TLS, WS bridge, SSH terminal)
- `/tmp/mac-mgmt-daemon.log` — daemon (event loop, SSH server, services)
- `/tmp/mac-mgmt-server.log` — server (API, web UI)

Check these logs when diagnosing connection issues, TLS handshake failures, or WebSocket problems.

## Chaos testing: run `/chaos-test` after significant daemon changes

After making non-trivial changes to the daemon (event loop, heartbeat logic, SSE handling, config reload, service management, relay integration), run `/chaos-test` to check for regressions under fault injection. The chaos test suite exercises the daemon with randomized endpoint failures, rapid SSE pushes, multi-daemon coordination, and supervisor lifecycle — catching race conditions and error handling bugs that unit tests miss. A quick run (`/chaos-test 5`) takes under two minutes; a thorough sweep (`/chaos-test 30`) takes about ten.
