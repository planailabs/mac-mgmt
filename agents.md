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
