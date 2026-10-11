#!/bin/sh
# Build the daemon as a static musl binary — the form the fleet's instances
# can actually run. An instance has a plain Ubuntu userland and no /nix/store,
# so a glibc build linked against the nix glibc is dead on arrival: its
# interpreter is a nix-store path that does not exist there.
#
# Mirrors what CI's xzar.sh does: route the musl target through cargo-zigbuild
# (without it, host-built C objects pull in glibc fortify symbols like
# __memcpy_chk and the link fails), and provide an empty libdl.a because musl
# has dlopen built in but libloading still asks for -ldl.
#
# Run inside the repo's nix dev shell.
set -eu
. "$(dirname "$0")/lib.sh"

cd "$checkout"

DL_STUB=$(mktemp -d)
SHIM=$(mktemp -d)
trap 'rm -rf "$DL_STUB" "$SHIM"' EXIT
ar rcs "$DL_STUB/libdl.a"

real_cargo=$(command -v cargo)
zigbuild=$(command -v cargo-zigbuild)
cat > "$SHIM/cargo" <<SHIMEOF
#!/usr/bin/env bash
# Route musl/darwin targets through cargo-zigbuild, pass everything else on.
use_zig=false
prev=""
args=()
for arg in "\$@"; do
  case "\$prev" in
    --target) case "\$arg" in *apple*|*darwin*|*linux-musl*) use_zig=true ;; esac ;;
  esac
  case "\$arg" in
    --target=*apple*|--target=*darwin*|--target=*linux-musl*) use_zig=true ;;
    +*) prev="\$arg"; continue ;;
  esac
  prev="\$arg"
  args+=("\$arg")
done
if \$use_zig; then
  CARGO="$real_cargo" exec "$zigbuild" "\${args[@]}"
else
  exec "$real_cargo" "\$@"
fi
SHIMEOF
chmod +x "$SHIM/cargo"
export PATH="$SHIM:$PATH"
export RUSTFLAGS="${RUSTFLAGS:-} -L $DL_STUB"

echo "note: run this inside the repo's nix dev shell (cargo, dx, cargo-zigbuild)"
cd "$checkout/server"
dx build --package mac-mgmt --release --embed \
  @client --platform web --no-default-features --features web \
  @server --platform server --target x86_64-unknown-linux-musl \
    --features self-update,services,relay,memvault

echo
echo "built: $checkout/target/x86_64-unknown-linux-musl/server-release/mac-mgmt"
