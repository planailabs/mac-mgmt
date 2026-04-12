# Skills and Bundles

Skills and bundles are the primary way to deploy software to clusters.

## Skills

A skill is a Nix-packaged tool or application. Each skill has:

- A unique **slug** (identifier)
- A human-readable **name** and **description**
- One or more **channels** (e.g. `stable`, `unstable`) representing different release tracks

When assigned to a cluster, the daemon resolves the skill's Nix store path and installs it.

### Assigning skills

Skills can be assigned to a cluster in two ways:

1. **Directly** — assign a specific skill channel to the cluster
2. **Via bundle** — assign a bundle that includes the skill

Direct assignments take precedence over bundle assignments. If the same skill appears both directly and through a bundle, the direct assignment wins.

## Bundles

A bundle groups multiple skill channels together for convenient assignment. Instead of assigning 10 skills individually to every cluster, create a bundle and assign that.

Bundles have:

- A unique **slug**
- A **name** and **description**
- A list of **skill channels** (bundle items)

## MCP Servers

MCP (Model Context Protocol) servers provide integrations and tools that OpenClaw can use. Each MCP server has:

- A unique **slug**
- A JSON **configuration** (passed to the MCP server at startup)
- A list of **Nix packages** required to run the server

### MCP Bundles

Similar to skill bundles, MCP bundles group multiple MCP servers for bulk assignment.

### Transitive dependencies

Skill channels can declare MCP server dependencies. When a skill is assigned to a cluster, its MCP server dependencies are automatically included. These transitive dependencies have the lowest precedence:

1. **Direct assignment** (highest priority)
2. **Bundle assignment**
3. **Transitive dependency** (lowest priority)

If an MCP server is already assigned directly or via bundle, the transitive dependency is skipped.

## Catalog visibility

Both skills and MCP servers can be marked as **hidden from the public catalog**. Hidden items are still functional but won't appear in catalog listings for non-admin users.

## Syncing from xzar

Skills and their channels can be synced from an external xzar service. The sync process:

1. Fetches all pins from xzar matching the `skill/{slug}/{channel}/{arch}` pattern
2. Creates missing skills and channels
3. Removes channels and skills that no longer exist in xzar

Use the **Sync from xzar** button on the Skills page to trigger this.
