#!/bin/sh
# Commit the fleet-deploy genericity change (this task's shell cannot pass a
# multi-line message inline).
set -eu
cd "$(dirname "$0")/.."
git add -A fleet-deploy AGENTS.md .gitignore
git -c user.name=reagent -c user.email=reagent@localhost commit -q -F - <<'MSG'
fleet-deploy: make the deploy scripts generic, config from a gitignored .env

Every script now sources fleet-deploy/.env (site-specific: hosts, addresses,
ports, paths, tokens; gitignored) and ships a committed .env.example
documenting each key. The three service configs became templates with
@PLACEHOLDER@ markers that setup.sh renders from the .env, so nothing
site-specific lives in a committed file. Adds setup.sh (one-time: certs,
databases, signing key, configs, seed), publish-daemon.sh and a README, and
drops the one-off debug scripts the generic ones supersede.

AGENTS.md gains the rule so this does not regress.
MSG
git log --oneline -3
