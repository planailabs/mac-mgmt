---
name: Cluster Comparison
desc: Compare a failing instance against healthy ones to find configuration drift
---

Compare a failing instance against healthy instances in the same cluster to identify
configuration drift or instance-specific issues.

## When to use

- The root cause is unclear from logs and config alone.
- Other instances in the cluster are healthy — what's different?
- After a config change: verify the fix matches what works elsewhere.

## Procedure

1. **Identify healthy instances**
   - The system prompt lists other instances and their health status.
   - Pick a healthy instance to use as the reference.

2. **Compare configurations**
   - `read_file("openclaw-config", "openclaw.json")` — local config of the failing instance.
   - `fetch_cluster_logs(instance_prefix: "<healthy>", service: "<name>", n: 50)` — logs
     from the healthy instance to see what normal operation looks like.
   - `run_cluster_command(instance_prefix: "<healthy>", command: "ollama-list")` — what
     models are installed on the healthy instance.

3. **Compare behavior**
   - `run_cluster_command(instance_prefix: "<healthy>", command: "openclaw-health")` — does
     the healthy instance pass the same health check?
   - Compare log patterns: are they running the same model? Same config?

4. **Apply findings**
   - If the healthy instance has a different config, align the failing instance to match.
   - If the difference is in installed models, pull the missing model.
   - If the difference is environmental (different hardware, different OS), note it in
     your diagnosis — it may require a different config rather than an identical one.

## Notes

- `run_cluster_command` and `fetch_cluster_logs` work via the relay — they require the
  target instance to be online and connected.
- Instance prefixes are the first 12 characters of the instance ID.
- Not all instances have the same shell commands or file tunnels — check availability
  before assuming a command exists on the target.
