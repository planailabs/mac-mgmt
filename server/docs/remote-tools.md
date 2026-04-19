---
audience: user
---

# Remote Tools

mac-mgmt provides browser-based tools for interacting with individual daemon instances through the relay. All tools are accessed from the **Fleet** dashboard by clicking on an instance.

## Prerequisites

All remote tools require:

- The instance must have `relay.url` configured (see [Configuration Reference](/docs/configuration-reference))
- The instance must be connected to the relay (visible in the Fleet dashboard)
- You need at least **read** access to the instance's cluster (write access for file editing)

The web UI automatically mints a short-lived proxy token (6-hour expiry) when you open any remote tool page. No manual token management is required.

## Log viewer

The log viewer streams service logs from a daemon instance in real time.

### Usage

1. Open the instance detail page from the Fleet dashboard
2. Select the **Logs** tab
3. Optionally filter by service using the dropdown
4. Click **Start Tailing** for continuous streaming, or **Fetch Latest** for a one-shot snapshot

The viewer fetches the most recent 500 log lines and then polls for new lines every 2 seconds while tailing is active. Click **Stop** to pause tailing, or **Clear** to reset the output.

## Shell commands

The shell commands page lets you run predefined commands on a daemon instance. Commands are registered by each managed service and grouped by service name.

### Usage

1. Open the instance detail page from the Fleet dashboard
2. Select the **Shell** tab
3. Find the command you want to run
4. If the command requires an argument, fill in the input field
5. Click **Run**

Output streams back in real time and is displayed in a terminal-style view. Commands are defined by the daemon's managed services — you cannot run arbitrary shell commands through this interface.

## File editor

The file editor provides browser-based editing of configuration files on a daemon instance. It supports both single-file and directory-style file tunnels.

### Usage

1. Open the instance detail page from the Fleet dashboard
2. Select the **Files** tab
3. Select a file or expand a directory in the left panel
4. Edit the file content in the right panel
5. Click **Save** to write changes back to the instance

### Features

- **Conflict detection** — the editor tracks file modification times. If the file changes on disk between your load and save, the save is rejected with a conflict error. Reload and retry in that case.
- **Read-only files** — some file tunnels are marked read-only and cannot be saved through the editor.
- **Binary files** — binary files are detected automatically and displayed as a placeholder instead of attempting to render them.

### Important notes

Changes made through the file editor apply to the **individual instance only** and are not synced across the cluster. For settings that should be consistent across all instances, use the cluster configuration editor instead. See [Configuration Reference](/docs/configuration-reference) for cluster-level settings.

## Related

- [Remote SSH](/docs/remote-ssh) — terminal access via SSH through the relay
- [Fleet Monitoring](/docs/fleet-monitoring) — heartbeat status and notifications
- [Healer Agent](/docs/healer) — AI-powered automated diagnosis and remediation
