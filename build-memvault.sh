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

RELEASE="${RELEASE:-1}"
if [ "$RELEASE" = "1" ]; then
  DX_PROFILE="--release"
else
  DX_PROFILE=""
fi

# ── 1. Tailwind CSS ─────────────────────────────────────────────────────
echo "▸ Building Tailwind CSS…"
(cd "$WEB_DIR" && npm run tailwind:build)

# ── 2. Dioxus fullstack build ───────────────────────────────────────────
# Use @client/@server overrides so the WASM client only gets the web feature
# (avoiding native deps like tokio/mio) while the server gets all features
# for a fully functional daemon binary.
echo "▸ Building Dioxus fullstack (client + server)…"
dx build --package mac-mgmt $DX_PROFILE --embed \
  @client --platform web --no-default-features --features web \
  @server --platform server

echo "✓ memvault-web built (assets embedded in server binary)"
