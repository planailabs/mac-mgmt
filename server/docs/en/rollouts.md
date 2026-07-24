---
audience: admin
---

# Rollouts

Rollouts provide staged, controlled updates across your fleet. They let you push daemon version upgrades and nixpkgs pin changes to groups of clusters in sequence. Managing rollouts requires admin access.

## Concepts

### Rollout groups

A rollout group is a named set of clusters. Create groups to represent deployment stages such as:

- **Canary** — a single test machine
- **Staging** — internal or low-risk clusters
- **Production** — the full fleet

Manage groups under **Rollout Groups** in the navigation. Each group has a name, description, and a set of member clusters.

### Rollouts

A rollout defines:

- A **target version** — the daemon version clusters should update to
- A **nixpkgs commit** (optional) — pin clusters to a specific nixpkgs revision
- One or more **stages** — each targeting a rollout group, executed in order

### Stages

Each stage targets a rollout group and has a status:

- `rolling` — actively deploying to this group's clusters
- `paused` — deployment halted; can be resumed
- `completed` — all clusters in this group have been updated

## Workflow

1. **Create rollout groups** — organize clusters into deployment tiers
2. **Create a rollout** — set the target version and/or nixpkgs commit, add stages in order
3. **Start the rollout** — the first stage begins rolling out
4. **Monitor** — watch the rollout detail page for progress
5. **Advance** — when the current stage looks good, advance to the next stage
6. **Pause/resume** — pause deployment if issues arise, resume when resolved
7. **Complete** — mark the rollout as done when all stages finish

## How it works

When a daemon checks for updates (`GET /api/update`), the server checks:

1. Is there an active rollout with a `rolling` stage that includes this cluster?
2. If yes, return the rollout's target version
3. If no, fall back to the cluster's pinned version
4. If no pinned version, fall back to the latest daemon version

The same precedence applies to nixpkgs pins (`GET /api/nixpkgs`).

## Best practices

- Start with a small canary group to catch issues early
- Use `upgrade_window` in the daemon config to control when upgrades happen
- Monitor the Fleet dashboard during rollouts to watch for unhealthy services
- Pause immediately if a stage shows problems before advancing
