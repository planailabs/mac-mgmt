# Agent Instructions

## Configuration

When changing default values in config structs (`OllamaConfig`, `OpenClawConfig`, etc.), always update `config.example.toml` to reflect the new defaults.

## Daemon: Running external commands

Always use `crate::cmd::output_with_timeout()` instead of `Command::new(...).output()` for any external command executed during the daemon's event loop (health checks, connectors, repair). Bare `.output()` has no timeout and can hang the daemon indefinitely. Use `cmd::DEFAULT_TIMEOUT` (30s) unless the command is known to be slow (e.g. `openclaw doctor --fix` uses 60s).

## mac-mgmt-services: protocol changes must be two-way backwards compatible

The supervisor and the daemon are upgraded independently (the supervisor keeps running across daemon self-updates, and a new daemon will commonly talk to an older supervisor and vice-versa). Any change to `mac-mgmt-services/src/protocol.rs` must stay compatible in both directions:

- New fields on existing variants go behind `#[serde(default)]` (readers that don't know them ignore them; readers that expect them tolerate their absence).
- When you replace a field, keep the old one on the wire during the transition and have the new side fall back to it when the new one is empty/missing (see the `statuses` / `names` pair on `Response::Services`).
- Don't rename or remove existing request/response variants; add new ones and keep handling the old shape.
- When in doubt, test with one side on the old protocol and one on the new.

Mark every compat shim with a `// compat: added YYYY-MM-DD, removable after YYYY-MM-DD` comment (three months out) so a later cleanup pass can delete shims confidently instead of guessing whether something out in the wild still needs them.

## Daemon: verify builds across feature combinations

When changing daemon code, ensure it compiles not only with default features but also with these additional feature combinations:

- `--no-default-features` (no features at all)
- `--no-default-features --features self-update` (only self-update)
- `--all-features` (all features enabled)
- `--no-default-features --features services,relay,self-update` (non-default features enabled alongside defaults)

Run `cargo check -p mac-mgmt` with each combination to catch gating issues early. Code behind a feature gate must not reference items from another feature without the appropriate `#[cfg(feature = "...")]` guard.

## Server: database migrations are append-only

Never modify an existing migration file in `server/migrations/`. Migrations that have already been applied to a database cannot be re-run, so editing them has no effect on deployed instances and causes checksum mismatches. Always create a new migration with the next sequence number instead (e.g. if `033_*.sql` exists, create `034_*.sql`).
