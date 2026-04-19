---
name: Connectivity Diagnostics
desc: Diagnose node offline, relay disconnects, and network connectivity issues
---

Diagnose and handle node connectivity issues — daemon offline, relay disconnects,
network problems.

## Symptoms

- `check_node_online` returns false.
- Tool calls fail with 502/503/504 errors.
- Relay client reports "Daemon disconnected".

## Procedure

1. **Check node status**
   - `check_node_online` — is the daemon connected to the relay?
   - If offline, `wait_for_node(timeout: 300)` — wait up to 5 minutes for reconnection.
   - The daemon may be rebooting, self-updating, or having network issues.

2. **While waiting**
   - `get_probe_status` — heartbeat data may still be recent even if the relay link is down.
   - `get_service_state` — the server may have cached state from the last heartbeat.
   - Check if other instances in the cluster are also affected — if so, it's likely a
     network or relay issue, not a single-node problem.

3. **Node came back online**
   - `fetch_logs(n: 50)` — check what happened during the outage.
   - Look for: self-update messages, reboot indicators, network errors, OOM kills.
   - If the node self-updated, services should restart automatically.

4. **Node stayed offline**
   - If `wait_for_node` times out, the node is down for an extended period.
   - `staff_ping(category: "network")` with the duration of the outage and any context
     from the last heartbeat.
   - `pin("diagnosis", ...)` documenting the connectivity failure.
   - `set_phase("needs_human_attention")`.

## Notes

- The relay client has built-in retry logic: on 502/503/504 it waits up to 10 minutes
  polling every 5 seconds. You don't need to implement your own retry loop for tool calls.
- A node going offline during a healer session triggers `AwaitingRetry` state — the session
  will auto-resume when the node reconnects.
- Don't confuse relay connectivity (daemon ↔ relay server) with service health (services
  on the node itself). A node can be online with unhealthy services, or offline with
  services that were fine before disconnection.
