# MCP server

A Model Context Protocol server definition in the catalog: slug, name, runtime
configuration, and nix package dependencies. MCP servers are assigned to
clusters (directly or via MCP bundles) and synced to daemons, where agent
tooling (skills, OpenClaw, the healer) can use them.
