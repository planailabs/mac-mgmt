# Agent Instructions

## Configuration

When changing default values in config structs (`OllamaConfig`, `OpenClawConfig`, etc.), always update `config.example.toml` to reflect the new defaults.

## Daemon: Running external commands

Always use `crate::cmd::output_with_timeout()` instead of `Command::new(...).output()` for any external command executed during the daemon's event loop (health checks, connectors, repair). Bare `.output()` has no timeout and can hang the daemon indefinitely. Use `cmd::DEFAULT_TIMEOUT` (30s) unless the command is known to be slow (e.g. `openclaw doctor --fix` uses 60s).
