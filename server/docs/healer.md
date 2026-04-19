---
audience: user
---

# Healer Agent

The Healer is an AI-powered agent that automatically diagnoses and remediates issues on fleet instances. It connects to a daemon instance through the relay, inspects service health, runs shell commands, reads log files, and applies fixes.

## Starting a session

Open the **Fleet** dashboard, click on an instance, and select the **Healer** tab. The page shows which services are unhealthy and lists previous sessions for this instance.

To start a new session:

1. Optionally type instructions describing the problem or what to investigate
2. Click **Start Healing**
3. The agent opens a live-streaming session

If no instructions are provided, the agent auto-diagnoses based on current service health and system state.

## Session lifecycle

Each session progresses through a series of states:

| State | Description |
|-------|-------------|
| Initializing | Agent is connecting and gathering context |
| Diagnosing | Agent is investigating the issue |
| Remediating | Agent is applying fixes |
| Verifying | Agent is confirming the fix worked |
| Done | Session completed successfully |
| Paused | Session paused by user or token budget |
| Failed | Session ended with an error |
| Cancelled | Session was manually cancelled |
| Needs Human Attention | Agent cannot resolve the issue automatically |

During an active session you can **Pause**, **Resume**, or **Cancel** it at any time.

## Pinned slots

As the agent works, it pins structured summaries to the top of the session view:

- **Diagnosis** — what the agent found wrong
- **Remediation Plan** — what the agent intends to do
- **Final Report** — summary of actions taken and outcome

These pins give a quick overview without reading the full conversation.

## Staff pings

When the healer encounters an issue it cannot resolve automatically, it creates a **staff ping** — an actionable notification for a human operator. Staff pings are categorized:

| Category | Typical cause |
|----------|---------------|
| `hardware` | Hardware failure detected |
| `network` | Network connectivity issue |
| `disk_space` | Disk full or nearly full |
| `config_error` | Invalid configuration |
| `service_crash` | Repeated service crashes |
| `model_issue` | LLM model download or runtime failure |
| `permission` | File or process permission problem |
| `dependency` | Missing dependency |
| `security` | Security-related concern |
| `performance` | Performance degradation |

Staff pings appear inline in the healer session view and are also collected in the centralized **Staff Pings** page (accessible from the Admin sidebar). From either location you can mark a ping as resolved.

## Viewing past sessions

The healer page for each instance lists the 20 most recent sessions with their final state and creation time. Click any session to view its full conversation history and pinned slots.

## Requirements

- The instance must be connected to the relay (i.e. `relay.url` is configured)
- You need **write** access to the instance's cluster
- The server must have the healer subsystem enabled
