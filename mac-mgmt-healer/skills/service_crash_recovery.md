---
name: Service Crash Recovery
desc: Diagnose and recover from service crashes, crash loops, and port conflicts
---

Diagnose and recover from service crashes, including crash loops and port conflicts.

## Symptoms

- Probe status shows UNHEALTHY with `connection` error class (connection refused).
- Logs show `EADDRINUSE` (port conflict) or repeated crash/restart cycles.
- Service is down but the daemon supervisor should be restarting it.

## Procedure

1. **Gather crash evidence**
   - `fetch_logs(service: "<name>")` — look for crash messages, stack traces, OOM kills.
   - `get_probe_history(service: "<name>", limit: 10)` — check when failures started.
   - `get_system_sample` — look for resource exhaustion (memory, disk, thermal).
   - `get_inventory` — check hardware specs to understand capacity.

2. **Identify root cause category**

   **Port conflict (EADDRINUSE)**:
   - A previous process is still holding the port. The supervisor may be trying to start
     a new instance while the old one hasn't fully shut down.
   - Look for `openclaw gateway stop` or equivalent shell command.
   - If available, run it to kill the stuck process, then wait for the supervisor to restart.
   - If not available, use `staff_ping(category: "service_crash")` to alert admins.

   **Port conflict — openclaw gateway specific (EADDRINUSE on 18789)**:
   - There is NO `openclaw gateway stop` shell command available to the healer.
   - **Do NOT use `openclaw-doctor --fix`** — it tries to install as a systemd service
     and fails with "Permission denied" on D-Bus, leaving the gateway in a worse state.
   - **Do NOT use `service-restart openclaw`** — it calls doctor --fix internally.
   - The safest approach is to escalate via `staff_ping(category: "service_crash")`.
   - Sometimes the stuck process exits naturally. Check back with `fetch_logs` after a wait.

   **OOM / resource exhaustion**:
   - Check `get_system_sample` for high memory/swap usage.
   - Check if the model is too large for available RAM/VRAM.
   - Consider swapping to a smaller model (use `ollama_model_swap` skill).

   **Configuration error**:
   - Logs may show config parse errors or invalid settings.
   - Read the config file and compare with working instances via `fetch_cluster_logs`.
   - Fix the config and restart.

   **Dependency missing**:
   - Logs show "No such file or directory" or "command not found".
   - `staff_ping(category: "dependency")` — you can't install packages.

3. **Wait for recovery**
   - After fixing the cause, the daemon supervisor auto-restarts services.
   - `wait(30)` then check `get_service_state` or `get_probe_status`.
   - If still unhealthy after 2 minutes, investigate further.

4. **Verify**
   - `request_assessment` to trigger fresh probes.
   - `wait(60)` then `get_probe_status`.
   - Check that both liveness and functional probes pass.

## Notes

- Services are managed by the daemon supervisor, NOT by systemd. `systemctl` won't find them.
- The supervisor auto-restarts crashed services, so a transient crash may self-heal.
- If services are healthy in `get_service_state` but CLI health checks fail, it may be a
  timing issue — the supervisor has restarted the process but it hasn't fully initialized yet.
