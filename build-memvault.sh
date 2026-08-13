#!/usr/bin/env bash
# Build the memvault-web frontend via dx. Both client (WASM) and server
# (native daemon) are compiled from the same crate, sharing a cache —
# this guarantees hydration consistency.
#
# The --embed flag tells dx to bake the client's public assets into the
# server binary via dioxus-server's rust-embed integration.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$SCRIPT_DIR/memvault/crates/memvault-web"
MEMVAULT_MANIFEST="$WEB_DIR/Cargo.toml"

# The superproject exposes memvault's design checkout through ./design. Cargo
# keys path packages by their written path, not their resolved inode, so the
# nested memvault path and the superproject path otherwise become two distinct
# plan-ai-design packages in one lockfile. Rewrite the nested manifest for this
# build. Callers that run another workspace Cargo command can keep the canonical
# path until their lifecycle cleanup; other callers restore it on exit.
restore_memvault_manifest() {
  if [ -n "${MEMVAULT_MANIFEST_BACKUP:-}" ] && [ -f "$MEMVAULT_MANIFEST_BACKUP" ]; then
    cp "$MEMVAULT_MANIFEST_BACKUP" "$MEMVAULT_MANIFEST"
    rm -f "$MEMVAULT_MANIFEST_BACKUP"
  fi
}
MEMVAULT_MANIFEST_BACKUP="$(mktemp "${TMPDIR:-/tmp}/memvault-web-Cargo.toml.XXXXXX")"
cp "$MEMVAULT_MANIFEST" "$MEMVAULT_MANIFEST_BACKUP"
if [ "${MEMVAULT_KEEP_CANONICAL_MANIFEST:-0}" = "1" ]; then
  rm -f "$MEMVAULT_MANIFEST_BACKUP"
  MEMVAULT_MANIFEST_BACKUP=""
else
  trap restore_memvault_manifest EXIT
fi
python3 - "$MEMVAULT_MANIFEST" <<'PY'
from pathlib import Path
import sys

manifest = Path(sys.argv[1])
old = 'plan-ai-design = { path = "../../plan-ai-design" }'
new = 'plan-ai-design = { path = "../../../design" }'
text = manifest.read_text()
if text.count(old) != 1:
    raise SystemExit(f"expected exactly one canonical design dependency in {manifest}")
manifest.write_text(text.replace(old, new))
PY

# Cargo and dx both need a usable Cargo home. CI normally symlinks ~/.cargo to a
# shared cache volume; if that symlink is broken, dx's nested cargo-metadata run
# fails later with an opaque "failed to create directory ... File exists" error.
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export CARGO_HOME
if [ -L "$CARGO_HOME" ] && [ ! -e "$CARGO_HOME" ]; then
  echo "✗ CARGO_HOME points at a broken symlink: $CARGO_HOME -> $(readlink "$CARGO_HOME")" >&2
  echo "  Initialize the cache target before linking ~/.cargo." >&2
  exit 1
fi

RELEASE="${RELEASE:-1}"
if [ "$RELEASE" = "1" ]; then
  DX_PROFILE="--release"
else
  DX_PROFILE=""
fi

# ── 1. Cargo metadata preflight ─────────────────────────────────────────
# dx has its own cargo-metadata watchdog.  On loaded CI runners that watchdog
# can expire before emitting useful Cargo diagnostics, so resolve metadata once
# up front with a normal timeout.  This both warms Cargo's metadata cache for dx
# and makes lock/network failures point at the actual failing command.
METADATA_TIMEOUT="${CARGO_METADATA_TIMEOUT:-180}"
echo "▸ Preflighting cargo metadata (timeout: ${METADATA_TIMEOUT}s)…"
set +e
timeout "$METADATA_TIMEOUT" cargo metadata --format-version=1 --locked --no-deps >/dev/null
status=$?
set -e
if [ "$status" -ne 0 ]; then
  echo "✗ cargo metadata preflight failed (exit ${status})" >&2
  echo "Active cargo/rustc processes:" >&2
  ps -ef | grep -E '[c]argo|[r]ustc|[r]ustdoc' >&2 || true
  exit "$status"
fi

# ── 2. Tailwind CSS ─────────────────────────────────────────────────────
echo "▸ Building Tailwind CSS…"
(cd "$WEB_DIR" && npm run tailwind:build)

# ── 3. Dioxus fullstack build ───────────────────────────────────────────
# Use @client/@server overrides so the WASM client only gets the web feature
# (avoiding native deps like tokio/mio) while the server gets all features
# for a fully functional daemon binary.
echo "▸ Building Dioxus fullstack (client + server)…"
DX_LOG="${TMPDIR:-/tmp}/dx-build-memvault.$$.log"
DX_CMD=(dx build --package mac-mgmt)
if [ -n "$DX_PROFILE" ]; then
  DX_CMD+=("$DX_PROFILE")
fi
DX_CMD+=(--embed
  @client --platform web --no-default-features --features web
  @server --platform server)
set +e
"${DX_CMD[@]}" 2>&1 | tee "$DX_LOG"
status=${PIPESTATUS[0]}
set -e
if [ "$status" -ne 0 ]; then
  if grep -q "cargo metadata took too long" "$DX_LOG"; then
    echo "✗ dx timed out waiting for cargo metadata even after preflight." >&2
    echo "Active cargo/rustc processes:" >&2
    ps -ef | grep -E '[c]argo|[r]ustc|[r]ustdoc' >&2 || true
  fi
  exit "$status"
fi
rm -f "$DX_LOG"

echo "✓ memvault-web built (assets embedded in server binary)"
