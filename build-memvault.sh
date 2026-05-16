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
echo "▸ Building Dioxus WASM client…"
dx build --package memvault-web --platform web --no-default-features --features web $DX_PROFILE

# ── 3. Copy to daemon embed directory ────────────────────────────────────
echo "▸ Copying assets to $DIST_DIR"
rm -rf "$DIST_DIR"
cp -r "$DX_OUT" "$DIST_DIR"

# ── 4. Patch hydration to be resilient to SSR/client tree mismatches ────
# When the WASM is built separately and embedded in the daemon, hydration
# entry ordering can diverge. This patch makes hydrate_node skip nodes
# where the client VirtualDom doesn't have a matching ID instead of crashing.
echo "▸ Patching hydration for embedded mode…"
INTERP_JS="$DIST_DIR/wasm/snippets/dioxus-interpreter-js-"*/inline0.js
if ls $INTERP_JS 1>/dev/null 2>&1; then
  sed -i 's/hydrate_node(hydrateNode,ids){let split=hydrateNode.getAttribute("data-node-hydration").split(","),id=ids\[parseInt(split\[0\])\];/hydrate_node(hydrateNode,ids){let split=hydrateNode.getAttribute("data-node-hydration").split(","),id=ids[parseInt(split[0])];if(id===undefined)return;/' $INTERP_JS
  echo "  ✓ Patched hydrate_node for graceful mismatch handling"
else
  echo "  ⚠ Could not find interpreter JS to patch (hydration errors may occur)"
fi

echo "✓ memvault-web frontend built → $DIST_DIR"
