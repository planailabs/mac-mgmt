#!/usr/bin/env bash

set -euxo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export ENVIRONMENT=production

# ── Daemon binary build & upload ─────────────────────────────────────
# Build the daemon for each supported target, drop the binary into a
# bin/ dir, add it to the local nix store, and upload as
# `daemon/$version/$nixSystem` so the server's daemon-versions sync can
# index it and the daemon can `nix-store --realise` it on update.

DAEMON_VERSION="$(grep '^version' "$SCRIPT_DIR/daemon/Cargo.toml" | head -1 | cut -d'"' -f2)"
FEATURES="self-update,services,relay"

upload() {
  ~/.cargo/bin/xzar --server planai upload --pin "$1" --desc $(readlink -f "$2") --leave-after-abandon 1m "$2"
}

# Rust target ↔ nix system identifier
RUST_TARGETS=(
  x86_64-unknown-linux-musl
  aarch64-apple-darwin
)
nix_system_for() {
  case "$1" in
    x86_64-unknown-linux-musl) echo "x86_64-linux" ;;
    aarch64-apple-darwin)      echo "aarch64-darwin" ;;
    *) echo "unknown rust target: $1" >&2; exit 1 ;;
  esac
}

# Native linux build
# libloading (via dioxus→subsecond) emits #[link(name = "dl")] on Linux,
# but musl libc has dlopen/dlsym built-in — no separate libdl exists.
# Provide an empty stub archive so the linker resolves -ldl.
DL_STUB="$(mktemp -d)"
ar rcs "$DL_STUB/libdl.a"
export RUSTFLAGS="${RUSTFLAGS:-} -L $DL_STUB"
cargo build --release --target x86_64-unknown-linux-musl -p mac-mgmt --features "$FEATURES"
rm -rf "$DL_STUB"

# Darwin cross via zigbuild + macOS SDK from the flake
SDKROOT="$(nix build --no-link --print-out-paths "$SCRIPT_DIR#macosx-sdk")"
export SDKROOT
cargo zigbuild --release --target aarch64-apple-darwin -p mac-mgmt --features "$FEATURES"

upload_daemon_binary() {
  local rust_target="$1"
  local nix_system
  nix_system="$(nix_system_for "$rust_target")"
  local bin_src="$SCRIPT_DIR/target/${rust_target}/release/mac-mgmt"

  if [ ! -f "$bin_src" ]; then
    echo "missing daemon binary: $bin_src" >&2
    exit 1
  fi

  local stage
  stage="$(mktemp -d)"
  mkdir -p "$stage/bin"
  cp "$bin_src" "$stage/bin/mac-mgmt"
  chmod +x "$stage/bin/mac-mgmt"

  rm -f result
  STORE_PATH="$(nix-store --add "$stage")"
  ln -sf "$STORE_PATH" result
  upload "daemon/${DAEMON_VERSION}/${nix_system}" result
  rm -rf "$stage"
}

for t in "${RUST_TARGETS[@]}"; do
  upload_daemon_binary "$t"
done
