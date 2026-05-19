#!/usr/bin/env bash

set -euxo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export ENVIRONMENT=production

# ── Configure xzar plan.ai cache ────────────────────────────────────
xzar config add-server planai https://xzar.plan.ai "$XZAR_TOKEN"

# ── Daemon binary build & upload ─────────────────────────────────────
# Build the daemon for each supported target via dx, drop the binary
# into a bin/ dir, add it to the local nix store, and upload as
# `daemon/$version/$nixSystem` so the server's daemon-versions sync can
# index it and the daemon can `nix-store --realise` it on update.

DAEMON_VERSION="$(grep '^version' "$SCRIPT_DIR/daemon/Cargo.toml" | head -1 | cut -d'"' -f2)"

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
  upload "daemon/${DAEMON_VERSION}/${nix_system}" result
  rm -rf "$stage"
}

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

# Shim cargo so dx uses cargo-zigbuild for the macOS cross-compile.
# zigbuild handles cc-rs, assembly, and linking via zig's built-in
# cross-compilation — no manual CC/AR wrappers needed.
CARGO_SHIM="$(mktemp -d)"
REAL_CARGO="$(which cargo)"
ZIGBUILD="$(which cargo-zigbuild)"
cat > "$CARGO_SHIM/cargo" <<SHIM
#!/usr/bin/env bash
# Only use zigbuild for apple/darwin targets; pass through for wasm/native.
# dx calls "cargo rustc ..." so we invoke cargo-zigbuild directly (it
# accepts build/rustc/test/run subcommands natively).
use_zig=false
prev=""
# Strip +toolchain args (e.g. +nightly) — cargo-zigbuild doesn't support them.
args=()
for arg in "\$@"; do
  case "\$prev" in
    --target) [[ "\$arg" == *apple* || "\$arg" == *darwin* ]] && use_zig=true ;;
  esac
  case "\$arg" in
    --target=*apple*|--target=*darwin*) use_zig=true ;;
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

dx build --package mac-mgmt --release --embed \
  @client --platform web --no-default-features --features web \
  @server --platform server --target aarch64-apple-darwin \
    --features self-update,services,relay,memvault

export PATH="${PATH#"$CARGO_SHIM:"}"
rm -rf "$CARGO_SHIM"

upload_daemon_binary aarch64-apple-darwin
