#!/usr/bin/env bash
# Build the memvault-web frontend assets (Tailwind CSS + Dioxus WASM).
# Does NOT compile the daemon — call this before `cargo build -p mac-mgmt`.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$SCRIPT_DIR/memvault/crates/memvault-web"
DIST_DIR="$SCRIPT_DIR/daemon/memvault-web-dist"

RELEASE="${RELEASE:-1}"
if [ "$RELEASE" = "1" ]; then
  DX_PROFILE="--release"
  DX_OUT="$SCRIPT_DIR/target/dx/memvault-web/release/web/public"
else
  DX_PROFILE=""
  DX_OUT="$SCRIPT_DIR/target/dx/memvault-web/debug/web/public"
fi

# ── 1. Tailwind CSS ─────────────────────────────────────────────────────
echo "▸ Building Tailwind CSS…"
(cd "$WEB_DIR" && npm run tailwind:build)

# ── 2. Dioxus WASM client ───────────────────────────────────────────────
# --renderer web tells dx the renderer is known, preventing it from auto-
# adding the "web" feature (which would re-enable dioxus-web/hydrate via
# dioxus/fullstack). Only web-embedded is active → no hydration.
echo "▸ Building Dioxus WASM client…"
dx build --package memvault-web --platform web --renderer web \
  --no-default-features --features web-embedded $DX_PROFILE

# ── 3. Copy to daemon embed directory ────────────────────────────────────
echo "▸ Copying assets to $DIST_DIR"
rm -rf "$DIST_DIR"
cp -r "$DX_OUT" "$DIST_DIR"

echo "✓ memvault-web frontend built → $DIST_DIR"
