# Healer

The autonomous remediation agent. When an instance is unhealthy (or on
demand), a healer session diagnoses via read-only tools, optionally pauses for
human approval, then remediates and verifies. Sessions have phases
(diagnosing/remediating/verifying), token budgets, a validator-LLM guard on
risky tool calls, and full transcripts. Configured under [healer]; per-cluster
overrides exist.
