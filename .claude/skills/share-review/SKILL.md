---
name: share-review
description: Use when asked to triage pending memvault cross-cluster share proposals via the share-agent MCP. Approves, attenuates, rejects, or escalates each.
allowed-tools: mcp__share__share_inbox_list mcp__share__share_inspect mcp__share__share_approve mcp__share__share_reject
---

# Share Review Skill

You are reviewing pending memvault share proposals.

## Step 1: Triage
Run `share_inbox_list` to list pending proposals. For each:
- If `provisional_first_contact = true`, ALWAYS run `share_inspect` and verify the proposer's cluster admin-key signature chain. If unverifiable, `share_reject` with reason "first-contact identity not verifiable".
- If the proposer cluster is in the local `ClusterTrust` set, proceed to step 2.

## Step 2: Classification check
Run `share_inspect` on the proposal to check what classifications are present in the source bucket. If any envelope is `classification:confidential`, report that to the user before proceeding. Confidential data requires explicit human approval.

## Step 3: Decide
- `internal` data + trusted cluster + proposed [Read] only → `share_approve`.
- `internal` data + proposed [Write] → reject unless explicitly authorized by the user.
- `public` data → approve as proposed.
- Any uncertainty → ask the user before deciding.

Do not approve more than 10 proposals without surfacing a summary. Bias toward rejection when classification or trust is unclear.
