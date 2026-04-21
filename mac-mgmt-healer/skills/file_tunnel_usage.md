---
name: File Tunnel Usage
desc: Reference for correct file tunnel API usage — avoid common mistakes that waste rounds
---

Quick reference for using file tunnels correctly. Incorrect usage is the most common
agent mistake and wastes tool-call rounds.

## File tunnel types

- **File tunnel** (kind: "file"): Points to a single file. Read/write with just
  the tunnel name — NO path argument.
- **Directory tunnel** (kind: "directory"): Points to a directory. Must specify
  a path relative to the tunnel root.

## Common mistakes

| Mistake | Error | Fix |
|---------|-------|-----|
| `read_file("ollama-env", "ollama-env")` | "sub-paths not allowed for file tunnels" | `read_file("ollama-env")` — omit the path |
| `read_file("daemon-config", "config.json")` | "path not found" | `list_files("daemon-config")` first, use correct filename |
| `read_file("openclaw-config", "")` | error | `list_files("openclaw-config")` first |

## Discovery procedure

1. `list_file_tunnels` — see available tunnels and their types.
2. For directory tunnels: `list_files(tunnel_name)` — see contents.
3. For file tunnels: `read_file(tunnel_name)` — read directly (no path).

## Key tunnels on typical instances

- **ollama-env**: FILE tunnel — `read_file("ollama-env")` directly.
- **daemon-config**: DIRECTORY tunnel — contains `config.toml`, `ollama-env`, etc.
  Use `list_files("daemon-config")` then `read_file("daemon-config", "config.toml")`.
- **openclaw-config**: DIRECTORY tunnel — contains `openclaw.json` and subdirectories.
- **openclaw-skills**: DIRECTORY tunnel, read-only.
- **mcporter-config**: DIRECTORY tunnel, read-only.
