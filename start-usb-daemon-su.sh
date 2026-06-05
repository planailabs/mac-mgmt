#!/usr/bin/env bash
# Build the sovereign-AI USB daemon (memvault web UI + native overview) and run
# it locally as the `daemon` user, mirroring start-daemon-su.sh — but with the
# X display plumbed through so the overview desktop window actually shows up.
#
# Windowed by default. Set HEADLESS=1 to run without the window (daemon-style).
set -euo pipefail

# Debug build, same dx compilation cache as the WASM client (no hydration skew).
RELEASE=0 ./build-usb.sh

cp target/dx/mac-mgmt/debug/web/server /tmp/mac-mgmt
chmod 755 /tmp/mac-mgmt

# A stick/home dir the daemon user can write (HOME is repinned here by the usb
# command; supervisor socket, nix profile, config + caches all go here).
USB_HOME="${USB_HOME:-/tmp/usb-home}"
mkdir -p "$USB_HOME"
chown daemon:daemon "$USB_HOME" 2>/dev/null || chmod 0777 "$USB_HOME"

HEADLESS="${HEADLESS:-0}"

if [ "$HEADLESS" = "1" ] || [ -z "${DISPLAY:-}" ]; then
  # Headless: no window (or no X server to show one on).
  [ -z "${DISPLAY:-}" ] && [ "$HEADLESS" != "1" ] && \
    echo "ℹ no \$DISPLAY — running headless. Start from a desktop session to see the window." >&2
  sudo su -l daemon -s /bin/bash -c \
    "env RUST_BACKTRACE=1 RUST_LOG=debug INPROCESS_SERVICE_MANAGER=1 \
     /tmp/mac-mgmt usb --headless --home '$USB_HOME'" \
    | tee /tmp/mac-mgmt-usb-daemon.log
  exit 0
fi

# ── Windowed: plumb the X display through to the daemon user ─────────────
# Grant the daemon user access to this X server (server-interpreted auth, so no
# cookie is needed), and revoke it when we exit. Force the GTK/webkit X11
# backend so it works the same under Wayland (via XWayland).
GRANTED=0
if command -v xhost >/dev/null 2>&1; then
  if xhost +SI:localuser:daemon >/dev/null 2>&1; then GRANTED=1; fi
fi
cleanup() {
  [ "$GRANTED" = "1" ] && xhost -SI:localuser:daemon >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Make the inviting user's X cookie readable too (belt-and-suspenders for
# servers that still require it).
XAUTH="${XAUTHORITY:-$HOME/.Xauthority}"
[ -f "$XAUTH" ] && chmod a+r "$XAUTH" 2>/dev/null || true

echo "▸ launching overview window as 'daemon' on DISPLAY=$DISPLAY"
sudo su -l daemon -s /bin/bash -c \
  "env RUST_BACKTRACE=1 RUST_LOG=debug INPROCESS_SERVICE_MANAGER=1 \
   DISPLAY='$DISPLAY' XAUTHORITY='$XAUTH' GDK_BACKEND=x11 \
   WEBKIT_DISABLE_COMPOSITING_MODE=1 \
   /tmp/mac-mgmt usb --home '$USB_HOME'" \
  | tee /tmp/mac-mgmt-usb-daemon.log

# Provision once online first so it can also boot offline:
#   /tmp/mac-mgmt usb-prefetch --home "$USB_HOME"
