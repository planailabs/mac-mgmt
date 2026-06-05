#!/usr/bin/env bash
# Build the sovereign-AI USB daemon in a single dx fullstack pass that bundles
# BOTH UIs:
#   - the memvault web UI (WASM client → embedded into the server binary via
#     `--embed`, exactly like build-memvault.sh), and
#   - the native overview desktop app (the `usb-ui` feature → dioxus desktop /
#     wry), compiled into the same `mac-mgmt` server binary.
#
# Must run inside `nix develop` (the devShell provides webkitgtk/xdotool/etc.
# needed to link the wry overview). The result is one binary at
# target/dx/mac-mgmt/<profile>/web/server.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$SCRIPT_DIR/memvault/crates/memvault-web"

# Cargo and dx both need a usable Cargo home (see build-memvault.sh).
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export CARGO_HOME
if [ -L "$CARGO_HOME" ] && [ ! -e "$CARGO_HOME" ]; then
  echo "✗ CARGO_HOME points at a broken symlink: $CARGO_HOME -> $(readlink "$CARGO_HOME")" >&2
  echo "  Initialize the cache target before linking ~/.cargo." >&2
  exit 1
fi

# Server-side feature set. `usb-ui` implies `usb` (→ services + memvault + embed)
# plus the native overview crate; default features stay on.
USB_FEATURES="${USB_FEATURES:-usb-ui}"

RELEASE="${RELEASE:-1}"
if [ "$RELEASE" = "1" ]; then
  DX_PROFILE="--release"
else
  DX_PROFILE=""
fi

# ── 1. Cargo metadata preflight ─────────────────────────────────────────
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
echo "▸ Building Tailwind CSS (memvault web UI)…"
(cd "$WEB_DIR" && npm run tailwind:build)
# The overview desktop app inlines its own compiled CSS (design system + the
# shared config-ui editor classes); regenerate it so include_str! is current.
echo "▸ Building Tailwind CSS (overview)…"
(cd "$SCRIPT_DIR/mac-mgmt-overview" && npm install --no-audit --no-fund >/dev/null 2>&1 && npm run tailwind:build)

# ── 3. Dioxus fullstack build (client WASM + native server w/ usb-ui) ───
# @client gets only the `web` feature (WASM, no native deps); @server is the
# native daemon built with usb-ui so the overview desktop app links in, while
# --embed bakes the client assets into it for the memvault web UI.
echo "▸ Building Dioxus fullstack (memvault web UI + usb overview): features=${USB_FEATURES}…"
DX_LOG="${TMPDIR:-/tmp}/dx-build-usb.$$.log"
DX_CMD=(dx build --package mac-mgmt)
if [ -n "$DX_PROFILE" ]; then
  DX_CMD+=("$DX_PROFILE")
fi
DX_CMD+=(--embed
  @client --platform web --no-default-features --features web
  @server --platform server --features "$USB_FEATURES")
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

if [ "$RELEASE" = "1" ]; then BIN="target/dx/mac-mgmt/release/web/server"; else BIN="target/dx/mac-mgmt/debug/web/server"; fi
echo "✓ USB daemon built: $SCRIPT_DIR/$BIN"
echo "  (memvault web UI embedded + native overview app linked)"
