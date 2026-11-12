#!/usr/bin/env bash
# Self-check for dx_build's retry policy: bash scripts/dx-build-retry.test.sh
#
# Not part of any build. It stubs `dx` (a script of exit statuses, one per call)
# and checks the decisions that matter:
#   * a build that compiles is not retried;
#   * a compile error returns at once -- no retry;
#   * a dead rustc (exit 101) and a killed dx (SIGKILL, the OOM killer) are
#     retried, and the retries stop at DX_BUILD_ATTEMPTS;
#   * a real compile error after a kill still ends the loop.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"
export PATH="$work/bin:$PATH"
export NIX_BUILD_TOP=$work DX_BUILD_RETRY_DELAY=0 DX_BUILD_ATTEMPTS=3

# The stub plays one status per call from $DX_STUB_STATUSES (the last repeats)
# and counts its calls in $DX_STUB_CALLS.
cat >"$work/bin/dx" <<'stub'
#!/usr/bin/env bash
n=$(( $(cat "$DX_STUB_CALLS" 2>/dev/null || echo 0) + 1 ))
echo "$n" >"$DX_STUB_CALLS"
echo "dx build $* (call $n)" >&2
line=$(sed -n "${n}p" "$DX_STUB_STATUSES")
[ -n "$line" ] || line=$(tail -1 "$DX_STUB_STATUSES")
case $line in
ok) exit 0 ;;
error) echo 'error[E0425]: cannot find value `x` in this scope' >&2; exit 1 ;;
rustc-died) echo "process didn't exit successfully: cargo-auditable rustc … (exit status: 101)" >&2; exit 101 ;;
killed) echo 'Bundling app...' >&2; kill -9 $$ ;;
*) echo "unknown stub status $line" >&2; exit 99 ;;
esac
stub
chmod +x "$work/bin/dx"

fail=0
check() { # check <name> <statuses, comma-separated> <expected exit> <expected calls> [must appear in output]
    local name=$1 statuses=$2 want_rc=$3 want_calls=$4 want=${5:-} rc=0 calls
    printf '%s\n' ${statuses//,/ } >"$work/statuses"
    export DX_STUB_STATUSES=$work/statuses DX_STUB_CALLS=$work/calls
    rm -f "$work/calls" "$work/dx-build.log"
    ( source "$here/dx-build-retry.sh"; dx_build --fullstack ) >"$work/out" 2>&1 || rc=$?
    calls=$(cat "$work/calls" 2>/dev/null || echo 0)
    if [ "$rc" = "$want_rc" ] && [ "$calls" = "$want_calls" ] && { [ -z "$want" ] || grep -qF "$want" "$work/out"; }; then
        echo "ok   $name"
    else
        echo "FAIL $name: exit $rc (want $want_rc), $calls calls (want $want_calls)${want:+, output lacks \"$want\"}"
        sed 's/^/     /' "$work/out"
        fail=1
    fi
}

check "compiles: one call" ok 0 1
check "compile error: no retry" error 1 1
check "dead rustc: retried" rustc-died,ok 0 2
check "killed dx: retried" killed,ok 0 2
check "killed every time: stops at 3 attempts, says killed" killed 137 3 "was killed (exit 137"
check "compile error after a kill: stops" killed,error 1 2 "the cause is in this log"
check "log of every attempt is kept" killed,ok 0 2 "(call 1)"

if [ "$fail" -ne 0 ]; then
    echo "dx-build-retry: FAILED"
    exit 1
fi
echo "dx-build-retry: all checks passed"
