#!/usr/bin/env bash
# Build the memvault-web frontend assets (Tailwind CSS + Dioxus WASM) and
# the fullstack server binary via dx. Both client and server are compiled
# from the same crate with the same features, sharing a cache — this
# guarantees hydration consistency.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$SCRIPT_DIR/memvault/crates/memvault-web"
DIST_DIR="$SCRIPT_DIR/daemon/memvault-web-dist"

RELEASE="${RELEASE:-1}"
if [ "$RELEASE" = "1" ]; then
  DX_PROFILE="--release"
  DX_OUT="$SCRIPT_DIR/target/dx/mac-mgmt/release/web/public"
else
  DX_PROFILE=""
  DX_OUT="$SCRIPT_DIR/target/dx/mac-mgmt/debug/web/public"
fi

# ── 1. Tailwind CSS ─────────────────────────────────────────────────────
echo "▸ Building Tailwind CSS…"
(cd "$WEB_DIR" && npm run tailwind:build)

# ── 2. Dioxus fullstack build ───────────────────────────────────────────
# Use @client/@server overrides so the WASM client only gets the web feature
# (avoiding native deps like tokio/mio) while the server gets all features
# for a fully functional daemon binary.
echo "▸ Building Dioxus fullstack (client + server)…"
dx build --package mac-mgmt $DX_PROFILE \
  @client --platform web --no-default-features --features web \
  @server --platform server --features web

# ── 3. Copy to daemon embed directory ───────────────────────────────────
echo "▸ Copying assets to $DIST_DIR"
rm -rf "$DIST_DIR"
cp -r "$DX_OUT" "$DIST_DIR"

echo "✓ memvault-web frontend built → $DIST_DIR"
