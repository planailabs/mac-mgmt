---
name: chaos-test
description: Run Antithesis-style chaos and sampling tests against the daemon, identify bugs from failures, and fix them. Use when asked to find bugs, run chaos tests, or harden the daemon.
argument-hint: [rounds]
allowed-tools: Bash(cargo:*) Bash(git:*) Read Edit Write Grep Glob
---

# Chaos Test Skill

You are running the mac-mgmt simulation test suite to find and fix bugs in the daemon. The sim-tests crate (`sim-tests/`) contains an Antithesis-style chaos testing framework with a mock management server, fault injection, invariant checking, and seed-based reproduction.

The daemon runs with `services` and `relay` features enabled (but zero actual services and no real relay connection). This exercises the full code paths including ServiceManager, relay Manager, SSH key sync, and connectors.

## Step 1: Run the chaos + sampling tests

Run the full suite. Use the argument as the round count if provided, otherwise default to 20 chaos rounds and 30 sampling rounds.

```
CHAOS_ROUNDS=${0:-20} SAMPLE_ROUNDS=${0:-30} SAMPLE_EVENTS=12 RUST_LOG=warn cargo test -p sim-tests -- --nocapture 2>&1
```

This runs ALL test files: chaos (random faults, SSE stress, sustained faults), sampling (failure search with minimization), targeted (config churn, endpoint cycling, cascading failure), heartbeat, SSE reconnect, and relay integration.

Capture the full output. Look for:
- **FAILED** tests — these are bugs
- **seed=NNNN** values — these reproduce the exact failure
- **INVARIANT VIOLATION** — specific property that broke
- **TIMELINE** output — sequence of events leading to the failure

If all tests pass, report that and optionally increase rounds for deeper search.

## Step 2: Reproduce and understand failures

For each failure found, reproduce it deterministically:

```
CHAOS_SEED=<seed> RUST_LOG=info cargo test -p sim-tests --test chaos -- <test_name> --nocapture 2>&1
```

Read the timeline output carefully. The timeline shows:
- `[time] REQ endpoint` — mock server received a request
- `[time] PUSH event` — SSE push sent to daemon
- `[time] FAULT+ endpoint → status` — fault injected
- `[time] FAULT- endpoint` — fault cleared
- `[time] INV:FAIL invariant: detail` — invariant violation

Identify the root cause by tracing the sequence of events leading to the violation.

## Step 3: Diagnose the bug

Read the relevant daemon source code. The key files are:

- `daemon/src/daemon.rs` — main event loop, heartbeat sending, push handling, `run_sim()` entrypoint
- `daemon/src/server_push.rs` — SSE client with exponential backoff
- `daemon/src/config.rs` — config fetch and merge
- `daemon/src/skills.rs` — skills sync
- `daemon/src/mcp_servers.rs` — MCP server sync
- `daemon/src/assessment/mod.rs` — assessment/probe reporting
- `daemon/src/service_mgmt.rs` — ServiceManager (sim_init creates empty manager)
- `daemon/src/remote_ssh/mod.rs` — relay Manager (FIFO watcher skipped in sim)

Common bug patterns to look for:
- **Race conditions** in `tokio::spawn` fire-and-forget tasks
- **Missing error handling** that causes panics under fault injection
- **State corruption** when multiple push events arrive simultaneously
- **Timeout cascades** where one timeout causes downstream failures
- **Backoff bugs** where reconnection doesn't properly reset
- **Blocking calls** that stall the event loop (nix commands, supervisor RPC)

## Step 4: Fix the bug

Apply the fix to the daemon source code. Prefer minimal, targeted fixes:
- Don't refactor surrounding code
- Don't add unnecessary error handling for impossible cases
- Do add handling for the specific failure scenario found

## Step 5: Verify the fix

Re-run the failing seed to confirm it passes:

```
CHAOS_SEED=<seed> cargo test -p sim-tests --test chaos -- <test_name> --nocapture 2>&1
```

Then run a broader sweep to check for regressions:

```
CHAOS_ROUNDS=30 SAMPLE_ROUNDS=50 cargo test -p sim-tests 2>&1
```

## Step 6: Update tests if needed

If the bug revealed a gap in the invariant checkers or fault patterns:

- Add new invariants to `sim-tests/src/invariants.rs`
- Add new fault patterns to `sim-tests/src/scenarios.rs`
- Add targeted test cases to `sim-tests/tests/targeted.rs`
- Add relay-specific tests to `sim-tests/tests/relay.rs`

## Test files overview

| File | Tests | What it covers |
|------|-------|----------------|
| `tests/heartbeat.rs` | 3 | Basic heartbeat sending, fault recovery, multi-daemon |
| `tests/sse_reconnect.rs` | 3 | SSE push → skills/MCP/config sync |
| `tests/chaos.rs` | 3 | Random faults, SSE stress, sustained heartbeat fault |
| `tests/sampling.rs` | 1 | Failure sampling with automatic minimization |
| `tests/targeted.rs` | 3 | Config churn, endpoint cycling, cascading failure |
| `tests/relay.rs` | 4 | Unreachable relay, empty services, relay crash, SSH key push |

## Environment variables reference

| Variable | Default | Purpose |
|----------|---------|---------|
| `CHAOS_SEED` | random | Fix seed for deterministic reproduction |
| `CHAOS_ROUNDS` | 5 | Number of random chaos rounds |
| `SAMPLE_ROUNDS` | 10 | Number of sampling rounds |
| `SAMPLE_EVENTS` | 8 | Fault events per sampling round |
| `SAMPLE_DAEMONS` | 2 | Number of daemons per sampling round |
| `SAMPLE_SEED` | random | Base seed for sampling |
| `RUST_LOG` | warn | Log level (use `info` for reproduction) |

## Invariants checked

These properties must always hold regardless of fault schedule:

| Invariant | What it checks |
|-----------|---------------|
| `heartbeat_consistency` | services and tunnels fields are JSON arrays |
| `heartbeat_version_present` | version field is non-empty |
| `no_empty_instance_ids` | instance_id is never empty |
| `no_duplicate_instance_ids` | instance_id is valid 64-char hex |
| `heartbeat_temporal_order` | signed_at is monotonically increasing per instance |
| `heartbeat_liveness` | every daemon sends ≥1 heartbeat after settling |
