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

## Migrations

Never modify existing migrations. Always create new ones.
