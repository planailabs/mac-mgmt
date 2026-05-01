---
name: memvault
description: Use when the agent needs to store, retrieve, search, or link persistent memories across sessions. Provides a p2p knowledge graph with full-text search and file attachments.
tools:
  - plan-ai-memvault
---

# Memvault — Persistent P2P Memory

Store and retrieve information across conversations using a local-first, peer-to-peer memory store with knowledge graph capabilities, full-text search, and file attachments.

## When to Use

- Agent wants to remember facts, decisions, or preferences for future sessions
- Agent needs to recall prior context before answering a question
- User says "remember this", "store this", "save for later"
- User asks "do you remember", "what did we decide", "what do you know about"
- Agent needs to build a knowledge graph linking concepts together
- Agent wants to attach files or artifacts to a memory entry
- Agent needs to retract or correct previously stored information

## Available MCP Tools

### plan-ai-memvault

| Tool | Purpose |
|------|---------|
| `memvault_put` | Store a new memory entry (markdown text with optional title and tags) |
| `memvault_get` | Retrieve a specific memory by its CID (content-addressed ID) |
| `memvault_search` | Full-text search across all stored memories |
| `memvault_list` | List recent memories, optionally filtered by tags |
| `memvault_attach` | Attach a file (base64-encoded) to an existing memory entry |
| `memvault_graph_add` | Add a named node to the knowledge graph |
| `memvault_graph_link` | Create a labeled edge between two graph nodes |
| `memvault_graph_query` | Query the knowledge graph (neighbors, paths, subgraph) |
| `memvault_retract` | Mark a memory as retracted (soft-delete, preserves history) |
| `memvault_status` | Show store statistics (entry count, index size, peer status) |

## Workflow

### Storing a memory

```
memvault_put(
  text="The user prefers dark mode and uses neovim as their primary editor.",
  title="User preferences: editor and theme",
  tags=["user:preference", "tool:neovim"]
)
```

Returns a CID that uniquely identifies this memory entry.

### Searching memories

```
memvault_search(query="editor preferences", limit=5)
```

Returns matching entries ranked by relevance with snippets.

### Retrieving a specific memory

```
memvault_get(cid="bafy2bzaced...")
```

Returns the full content, metadata, and any attachments.

### Building a knowledge graph

```
memvault_graph_add(name="neovim", kind="tool", properties={"category": "editor"})
memvault_graph_add(name="dark-mode", kind="preference")
memvault_graph_link(from="user", to="neovim", relation="uses")
memvault_graph_link(from="user", to="dark-mode", relation="prefers")
```

### Querying the graph

```
memvault_graph_query(node="user", direction="outgoing", relation="uses")
```

Returns all nodes linked from "user" via the "uses" relation.

### Attaching a file

```
memvault_attach(cid="bafy2bzaced...", filename="config.lua", data="<base64>")
```

### Retracting a memory

```
memvault_retract(cid="bafy2bzaced...", reason="outdated — user switched to vscode")
```

The entry is marked retracted but remains in history for audit.

## Important Rules

1. **Search before storing** — avoid duplicating existing memories; update or link instead
2. **Use descriptive titles** — they improve search recall significantly
3. **Tag consistently** — use `scope:label` format (e.g., `user:preference`, `project:mac-mgmt`)
4. **Retract rather than delete** — retraction preserves history and is reversible
5. **Graph nodes are unique by name** — adding a node that already exists is a no-op
6. **CIDs are content-addressed** — the same content always produces the same CID
7. **Memories sync across peers** — stored data will replicate to other connected devices

## Memory Lifecycle

- **Put**: creates a new immutable entry, returns its CID
- **Search/List**: read-only queries against the local index
- **Attach**: adds a file blob linked to an existing entry
- **Retract**: marks an entry as superseded (still readable, excluded from search by default)
- **Graph operations**: build structured relationships between concepts

## Error Handling

- If `memvault_search` returns empty results: the query may be too specific, try broader terms
- If `memvault_get` fails with "not found": the CID may refer to an entry on a peer not yet synced
- If `memvault_put` fails: check disk space and data directory permissions
- If graph operations fail: ensure node names exist before linking
