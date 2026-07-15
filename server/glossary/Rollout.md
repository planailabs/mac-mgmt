# Rollout

A staged deployment of a daemon version across the fleet. A rollout targets a
version and progresses through stages (cohorts of clusters/groups) with health
gates between them; stages can be advanced, paused, resumed, rolled back, or
re-assessed. Cluster version pins override rollouts for individual clusters.
