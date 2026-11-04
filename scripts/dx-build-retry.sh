# Sourced by the two derivations that build with `dx` (overlay.nix for the
# daemon, server/package.nix for the server). Both used to call `dx build`
# inline; `dx_build` is that call plus two guards: the cause of a failure stays
# in the nix log, and a failure of one rustc does not throw the whole build away.
#
# Why: `dx` passes cargo's stderr lines that do not look like diagnostics to
# `status_build_message`, which logs at TRACE (src/build/context.rs), so at the
# default level they are dropped. A rustc that dies without a diagnostic of its
# own -- killed under memory pressure, or aborted on an allocation failure --
# therefore leaves only "cargo build finished with errors" in `nix log`. That is
# exactly the 2026-11-01 nightly: the daemon's rustc ran ~2 h and then exited
# 101 (cargo-auditable's rustc wrapper panics with "rustc was terminated by a
# deadly signal" when the real rustc dies), and the log said nothing else; the
# whole build was lost, and the compile error could not be told apart from an
# environment failure.
#
# dx_build <dx build args...>:
#   * runs `dx build` as before, keeping its output;
#   * if it failed for a transient reason (a rustc killed by a signal, a failed
#     allocation, an ICE, dx's cargo-metadata watchdog), runs it once more with
#     DIOXUS_LOG=info,dx=trace so the cause lands in the nix log. The target dir
#     is warm by then, so only the units that failed are rebuilt; no other
#     setting is changed (AGENTS.md: no CARGO_BUILD_JOBS throttling here).
#   * if it failed for any other reason (a compile error, a broken manifest) it
#     returns at once, exactly like the plain call did.
#
# A retry cannot hide a real failure: the failed attempt is in the log, and a
# build that genuinely does not compile still fails -- after the retry, with the
# compiler's diagnostics and full dx trace around them.

# dx_build_attempt <log> <append: 0|1> <dx args...> -- run dx, keep its output in
# <log> and on stdout/stderr, and return dx's own status. The subshell keeps the
# stdenv's `set -e` off the pipeline (the log has to be written even when dx
# fails) and passes dx's status, not tee's, out through PIPESTATUS.
dx_build_attempt() {
    local log=$1 append=$2 rc=0
    shift 2
    if [ "$append" = 1 ]; then
        ( set +e; dx build "$@" 2>&1 | tee -a "$log"; exit "${PIPESTATUS[0]}" ) || rc=$?
    else
        ( set +e; dx build "$@" 2>&1 | tee "$log"; exit "${PIPESTATUS[0]}" ) || rc=$?
    fi
    return "$rc"
}

dx_build() {
    local log rc
    log="${NIX_BUILD_TOP:-${TMPDIR:-/tmp}}/dx-build.log"

    rc=0
    dx_build_attempt "$log" 0 "$@" || rc=$?
    if [ "$rc" -eq 0 ]; then
        return 0
    fi

    # What a rustc that died without a diagnostic of its own looks like from
    # outside: cargo reports its wrappers with exit status 101 (a rustc killed by
    # a signal makes cargo-auditable's wrapper panic, which dx's default logging
    # hides), 101 for rustc's own fatal errors, or the allocation/ICE text if the
    # line did survive. A compiler diagnostic is exit status 1 -- not a retry.
    if ! grep -qE 'exit status: 101|signal: [0-9]+|terminated by a deadly signal|memory allocation of .* bytes failed|[Ii]nternal compiler error|cargo metadata took too long' "$log"; then
        return "$rc"
    fi

    echo "dx build failed transiently (a rustc died: the reason is above, or repeated in" \
        "the retry below). Retrying once with DIOXUS_LOG=${DX_RETRY_LOG:-info,dx=trace};" \
        "the target dir is warm, so only the units that failed are rebuilt." >&2
    rc=0
    DIOXUS_LOG="${DX_RETRY_LOG:-info,dx=trace}" dx_build_attempt "$log" 1 "$@" || rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "dx build failed again on the retry (exit $rc): the cause is in this log, not a transient one." >&2
    fi
    return "$rc"
}
