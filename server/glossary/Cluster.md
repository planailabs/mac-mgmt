# Cluster

A group of managed machines that share configuration, skills, MCP servers, and
daemon-version pins. Clusters belong to one or more organizations (which grant
users read/write access), and every daemon enrolls into exactly one cluster.
Cluster-level settings — config document, secrets, manual nix packages, healer
overrides, SSH keys, client certificates — are synced to all member daemons.
