#!/usr/bin/env bash

set -euo pipefail

: "${XZAR_TOKEN:?XZAR_TOKEN must be set for production artifact upload}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export ENVIRONMENT=production

# ── Configure xzar plan.ai cache ────────────────────────────────────
xzar config add-server planai https://xzar.plan.ai "$XZAR_TOKEN"

# ── Daemon binary build & upload ─────────────────────────────────────
# Build the daemon for each supported target via dx, drop the binary
# into a bin/ dir, add it to the local nix store, and upload as
# `daemon/rolling/$nixSystem` (every trunk build — the rolling channel)
# plus `daemon/$version/$nixSystem` for the semver pin. The semver pin
# is only uploaded once: re-uploading it on every trunk push would move
# the store path under clusters pinned to that version, defeating the
# pin. To re-cut a semver artifact, bump the version in daemon/Cargo.toml.

DAEMON_VERSION="$(grep '^version' "$SCRIPT_DIR/daemon/Cargo.toml" | head -1 | cut -d'"' -f2)"
# Fail hard if the pin list can't be fetched — falling back to "not
# published yet" would re-upload the semver pin and move pinned prod.
EXISTING_PINS="$(xzar --server planai list)"

# ── Tailwind CSS ────────────────────────────────────────────────────────
(cd "$SCRIPT_DIR/memvault/crates/memvault-web" && npm run tailwind:build)

# ── Build daemon for each target ────────────────────────────────────────

upload() {
  xzar --server planai upload --pin "$1" --desc "$(readlink -f "$2")" --leave-after-abandon 1m "$2"
}

nix_system_for() {
  case "$1" in
    x86_64-unknown-linux-musl) echo "x86_64-linux" ;;
    aarch64-apple-darwin)      echo "aarch64-darwin" ;;
    *) echo "unknown rust target: $1" >&2; exit 1 ;;
  esac
}

upload_daemon_binary() {
  local rust_target="$1"
  local nix_system
  nix_system="$(nix_system_for "$rust_target")"
  local bin_src="$SCRIPT_DIR/target/dx/mac-mgmt/release/web/server"

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
  upload "daemon/rolling/${nix_system}" result
  if printf '%s\n' "$EXISTING_PINS" | grep -qF "daemon/${DAEMON_VERSION}/${nix_system}"; then
    echo "pin daemon/${DAEMON_VERSION}/${nix_system} already published, skipping (rolling updated)"
  else
    upload "daemon/${DAEMON_VERSION}/${nix_system}" result
  fi
  rm -rf "$stage"
}

# ── Cross-compilation cargo shim ───────────────────────────────────────
# dx invokes `cargo rustc` directly for the server target. Use cargo-zigbuild
# for non-native targets so C build scripts and the final linker use the target
# C runtime instead of host glibc objects. Without this, musl release builds can
# link host-built zstd objects that reference glibc fortify symbols such as
# `__memcpy_chk`.
CARGO_SHIM="$(mktemp -d)"
REAL_CARGO="$(which cargo)"
ZIGBUILD="$(which cargo-zigbuild)"
cat > "$CARGO_SHIM/cargo" <<SHIM
#!/usr/bin/env bash
use_zig=false
prev=""
# Strip +toolchain args (e.g. +nightly) — cargo-zigbuild doesn't support them.
args=()
for arg in "\$@"; do
  case "\$prev" in
    --target) [[ "\$arg" == *apple* || "\$arg" == *darwin* || "\$arg" == *linux-musl* ]] && use_zig=true ;;
  esac
  case "\$arg" in
    --target=*apple*|--target=*darwin*|--target=*linux-musl*) use_zig=true ;;
    +*) prev="\$arg"; continue ;;
  esac
  prev="\$arg"
  args+=("\$arg")
done
if \$use_zig; then
  CARGO="$REAL_CARGO" exec "$ZIGBUILD" "\${args[@]}"
else
  exec "$REAL_CARGO" "\$@"
fi
SHIM
chmod +x "$CARGO_SHIM/cargo"
export PATH="$CARGO_SHIM:$PATH"
cleanup_cargo_shim() {
  export PATH="${PATH#"$CARGO_SHIM:"}"
  rm -rf "$CARGO_SHIM"
}
trap cleanup_cargo_shim EXIT

# ── Linux (musl) ────────────────────────────────────────────────────────
# libloading (via dioxus→subsecond) emits #[link(name = "dl")] on Linux,
# but musl libc has dlopen/dlsym built-in — no separate libdl exists.
# Provide an empty stub archive so the linker resolves -ldl.
DL_STUB="$(mktemp -d)"
ar rcs "$DL_STUB/libdl.a"
export RUSTFLAGS="${RUSTFLAGS:-} -L $DL_STUB"

dx build --package mac-mgmt --release --embed \
  @client --platform web --no-default-features --features web \
  @server --platform server --target x86_64-unknown-linux-musl \
    --features self-update,services,relay,memvault

rm -rf "$DL_STUB"
unset RUSTFLAGS

upload_daemon_binary x86_64-unknown-linux-musl

# ── macOS (aarch64) ─────────────────────────────────────────────────────
SDKROOT="$(nix build --no-link --print-out-paths "$SCRIPT_DIR#macosx-sdk")"
export SDKROOT

dx build --package mac-mgmt --release --embed \
  @client --platform web --no-default-features --features web \
  @server --platform server --target aarch64-apple-darwin \
    --features self-update,services,relay,memvault

upload_daemon_binary aarch64-apple-darwin
