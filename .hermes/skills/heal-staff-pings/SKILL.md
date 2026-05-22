---
name: heal-staff-pings
description: Use when asked to triage unresolved mac-mgmt healer staff pings by diagnosing and remediating each ping through the healer MCP tools.
version: 1.0.0
author: plan.ai
license: MIT
metadata:
  hermes:
    tags: [mac-mgmt, healer, staff-pings, operations]
    related_skills: []
user_invocable: true
---

# Heal Staff Pings

## Overview

Use the healer MCP tools to triage unresolved staff pings for mac-mgmt fleet instances. The goal is to diagnose each ping, attempt safe remediation, verify recovery, resolve the ping when fixed, and clearly report anything that still needs human attention.

This skill is intentionally diagnosis-first. Do not mark a ping resolved until you have verified the underlying condition improved. Do not create duplicate staff pings while handling existing pings.

## When to Use

Use this skill when the user asks you to:

- heal staff pings
- triage unresolved staff pings
- handle healer alerts
- resolve mac-mgmt staff notifications
- investigate issues surfaced through the healer MCP server

Do not use this as a generic service-restart checklist. Load the target instance's healer system prompt and follow the platform-specific remediation workflow for that session.

## Workflow

1. List unresolved pings.
   - In Hermes, call `mcp_healer_list_staff_pings()`.
   - In raw healer MCP terminology, this is `list_staff_pings(resolved: false)`.
   - If no unresolved pings exist, report that there is nothing to triage.

2. Group pings by target instance.
   - Group by `cluster_id` + `instance_id`.
   - Handle all pings for one instance in a single healer session when practical.
   - Avoid switching instances without ending the active healer session first.

3. For each instance group, create and prepare a healer session.
   - Call `mcp_healer_create_session(cluster_id, instance_id)`.
   - Give the session a short name with `mcp_healer_name_session()` once you understand the issue.
   - Call `mcp_healer_get_system_prompt()` immediately after session creation and follow its instructions.
   - Optionally check `mcp_healer_list_staff_pings()` again inside the session to avoid duplicating work if another operator resolved something.

4. Diagnose before remediating.
   - Read the ping's `message`, `category`, and any attached context.
   - Gather fresh state with appropriate healer tools such as:
     - `mcp_healer_get_service_state()`
     - `mcp_healer_get_probe_status()`
     - `mcp_healer_get_probe_history()`
     - `mcp_healer_fetch_logs()`
     - `mcp_healer_get_system_sample()`
     - `mcp_healer_get_inventory()`
   - Use built-in healer skills via `mcp_healer_list_builtin_skills()` and `mcp_healer_use_skill()` when one matches the failure mode.
   - Pin the diagnosis with `mcp_healer_pin(slot="diagnosis", ...)` once the root cause is clear.

5. Remediate safely.
   - Prefer targeted fixes from the healer system prompt or a built-in healer skill.
   - Use `mcp_healer_set_phase("remediating", ...)` before applying fixes.
   - Record the remediation plan with the appropriate healer pin before making risky changes.
   - Avoid broad restarts, config edits, package changes, or destructive commands unless the diagnosis justifies them.
   - If a fix can affect multiple services or instances, state the scope before applying it.

6. Verify recovery.
   - Use `mcp_healer_set_phase("verifying", ...)`.
   - Call `mcp_healer_request_assessment()` after remediation.
   - Wait long enough for probes to run, typically `mcp_healer_wait(30)` to `mcp_healer_wait(60)`.
   - Re-check with `mcp_healer_get_probe_status()` and any relevant logs or service state.
   - Only consider the ping fixed when the observed state matches the expected recovery.

7. Resolve fixed pings.
   - For each successfully fixed ping, call `mcp_healer_resolve_staff_ping(ping_id, resolved_by="admini")`.
   - If the tool reports the ping was already resolved, treat that as successful and mention it in the summary.
   - Pin a final report with `mcp_healer_pin(slot="final_report", ...)` summarizing what changed and what remains.

8. Escalate or pause when not fixable.
   - If you cannot fix the issue after reasonable attempts, do not create another staff ping for the same problem.
   - Use `mcp_healer_set_phase("needs_human_attention", ...)` when the active healer session cannot safely proceed.
   - Ask the human operator for guidance with a concise summary of diagnosis, attempted remediation, current evidence, and recommended next action.

9. End the session before moving on.
   - Call `mcp_healer_end_session()` after finishing an instance group.
   - Then continue with the next grouped instance.

10. Report the outcome.
    - List pings resolved.
    - List pings still unresolved and why.
    - Mention any actions taken, verification evidence, and any human follow-up needed.

## Important Rules

- Always call `mcp_healer_get_system_prompt()` after `mcp_healer_create_session()`.
- Always diagnose before attempting remediation.
- Always verify fixes with a fresh assessment and probe/status checks before resolving a ping.
- Do not silently skip unresolved pings.
- Do not create duplicate staff pings for issues already represented by existing pings.
- Do not mark a ping resolved just because a command succeeded; verify the service or host recovered.
- End the active healer session before creating a new one for another instance.

## Common Pitfalls

1. **Resolving too early.** A restart or config sync can succeed while probes still fail. Request an assessment and check probe status before resolving.
2. **Ignoring grouped pings.** Multiple pings for the same instance often share one root cause. Diagnose them together to avoid repeated or conflicting fixes.
3. **Skipping the healer system prompt.** The session prompt contains current platform rules and remediation boundaries. Load it every time.
4. **Creating duplicate escalations.** Existing staff pings are already escalations. Ask the operator for guidance instead of opening a new ping for the same issue.
5. **Leaving sessions open.** End the session before switching instances so later healer calls target the expected node.

## Verification Checklist

- [ ] Unresolved staff pings were listed.
- [ ] Pings were grouped by `cluster_id` + `instance_id`.
- [ ] A healer session was created for each handled instance.
- [ ] The healer system prompt was loaded and followed.
- [ ] Diagnosis evidence was gathered before remediation.
- [ ] Fixes were verified with fresh assessment/probe status.
- [ ] Resolved pings were marked resolved.
- [ ] Unresolved pings were reported with next steps.
- [ ] Each healer session was ended before moving to another instance.
