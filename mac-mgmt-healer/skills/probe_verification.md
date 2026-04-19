---
name: Probe Verification
desc: Correctly verify service health after changes — handle probe latency and stale results
---

Correctly verify service health after making changes. The probe system has latency —
results are not instant.

## The verification loop

After applying a fix:

1. `request_assessment` — tells the daemon to run probes immediately.
2. `wait(60)` — probes take time to execute, especially functional probes that make
   real LLM requests.
3. `get_probe_status` — check the results.

## Interpreting results

- **Heartbeat age**: Should be under 60s. If over 120s, the daemon may have disconnected.
- **Probe timestamps**: Compare with when you requested the assessment. If the probe
  timestamp is from before your request, the results are stale — wait longer.
- **Liveness vs functional**: Liveness checks if the process is running. Functional checks
  if the service actually works (e.g. can serve an LLM request). A service can be live
  but functionally broken.

## Common issues

- **Probes stuck showing UNHEALTHY**: The assessment hasn't completed yet. The daemon
  runs probes on its own schedule (~every few minutes). Your `request_assessment` triggers
  an immediate run, but it still takes time.
- **Liveness passes but functional fails**: The service is running but can't serve requests.
  Check logs for the specific error — model not loaded, wrong config, etc.
- **All probes stale**: If probe history shows no new entries after 2+ minutes post-assessment,
  the daemon may be having issues running probes. Check `get_service_state` as an alternative.

## Tips

- Don't spam `request_assessment` — one is enough, then wait.
- `get_service_state` (from heartbeat data) updates faster than probes and can confirm
  basic health while you wait for probe results.
- After verifying health, always `pin("final_report", ...)` and `set_phase("done")`.
- If probes don't update after 3 minutes, use `get_service_state` as the authoritative
  source and note the probe delay in your final report.
