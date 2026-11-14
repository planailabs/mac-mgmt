# Shared helpers for the CI scripts that pin build outputs to the plan.ai xzar
# cache. Source it, do not execute it:
#
#   . "$(dirname "${BASH_SOURCE[0]}")/scripts/xzar-cache.sh"
#
# shellcheck shell=bash

# xzar_add_server <server-name> <url> <token>
#
# `xzar config add-server` verifies the token against the server before it
# writes the config, so a single transient 5xx (or connection error) aborts the
# whole job before any build work runs: pipeline 3478 / job 8671 lost its 8h
# `build` budget 38s into `prebuild.sh` with "Server error (503 Service
# Unavailable)" from xzar.plan.ai and every later step skipped. The upload path
# already retries for exactly this reason, so do the same here — bounded — and
# put the endpoint, attempt number, and server response in the trace so a cache
# outage is recognizable instead of looking like a build failure.
xzar_add_server() {
  local name=${1:?xzar server name}
  local url=${2:?xzar server url}
  local token=${3:-}
  local attempt=1 max_attempts=8 delay=5 waited=0 out rc

  while :; do
    # `if out=$(...)` would set $? to 0 on the failing branch (an `if` with no
    # true condition exits 0), so capture the status explicitly.
    rc=0
    out=$(xzar config add-server "$name" "$url" "$token" 2>&1) || rc=$?
    if [ "$rc" -eq 0 ]; then
      [ -n "$out" ] && printf '%s\n' "$out"
      return 0
    fi
    printf 'xzar cache %s: config add-server attempt %d/%d failed (exit %d)\n' \
      "$url" "$attempt" "$max_attempts" "$rc" >&2
    printf '%s\n' "$out" | sed 's/^/  /' >&2
    if [ "$attempt" -ge "$max_attempts" ]; then
      printf 'xzar cache %s is unusable: token verification kept failing after %d attempts over ~%ds; no build work was started.\n' \
        "$url" "$max_attempts" "$waited" >&2
      return "$rc"
    fi
    printf 'xzar cache %s: retrying in %ds\n' "$url" "$delay" >&2
    sleep "$delay"
    waited=$((waited + delay))
    attempt=$((attempt + 1))
    # Cap the backoff so a long outage costs a few minutes, not the job budget.
    if [ "$delay" -lt 120 ]; then delay=$((delay * 2)); else delay=120; fi
  done
}
