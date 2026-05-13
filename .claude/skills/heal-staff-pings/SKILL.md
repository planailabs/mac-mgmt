---
description: Triage unresolved staff pings by diagnosing and remediating each one via the healer MCP tools.
---

# Heal Staff Pings

Use the healer MCP tools to triage all unresolved staff pings.

## Workflow

1. Call `list_staff_pings(resolved: false)` to get all unresolved pings.
2. Group pings by `instance_id` + `cluster_id` so you can handle multiple pings per instance in a single session.
3. For each instance group:
   a. Call `create_session` with the instance's `instance_id` and `cluster_id`.
   b. Call `get_system_prompt` and follow its instructions for diagnosis and remediation.
   c. For each ping on this instance:
      - Diagnose the issue described in the ping's `message` and `category`.
      - Attempt remediation using the healer tools.
      - If successful, call `resolve_staff_ping` for that ping.
      - If you cannot fix the issue after multiple attempts, **stop and ask the human operator on the console** for guidance. Do NOT create new staff pings.
   d. Call `end_session` before moving to the next instance.
4. Report a summary of what was resolved and what needs human attention.

## Important

- Always call `get_system_prompt` after `create_session` to understand the available tools and workflow.
- Follow the diagnosis-first approach: gather information before attempting fixes.
- Verify fixes with `request_assessment` and `get_probe_status` before marking pings resolved.
- If stuck, ask the human — do not create staff pings or silently skip issues.
