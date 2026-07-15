# Daemon

The mac-mgmt agent process running on each managed machine (also called an
instance). It sends heartbeats (services, system samples, versions) to the
server, syncs cluster configuration/skills/MCP servers, exposes file and shell
tunnels through the relay, and can self-update to a pinned or rolled-out
daemon version.
