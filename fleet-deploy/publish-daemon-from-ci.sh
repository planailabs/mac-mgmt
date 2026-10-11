#!/bin/sh
# Reproduce CI's daemon build locally and publish it into the local xzar.
#
# CI's xzar.sh is the authoritative recipe; this runs the same steps but stops
# after the musl binary, skips the macOS build, and uploads to the local cache
# instead of the production one. Unlike build-daemon-nix.sh (and `nix build
# .#default`), the output of this is a static musl binary, which is the only
# form a plain-Ubuntu fleet instance can execute: a glibc build's ELF
# interpreter is a /nix/store path that does not exist there.
#
# Run inside the repo's nix dev shell.
set -eu
. "$(dirname "$0")/lib.sh"

cd "$checkout"

# Cargo shim: route linux-musl through cargo-zigbuild so C build scripts and
# the linker use the musl runtime instead of host glibc objects. Without it the
# link fails on host-glibc fortify symbols (__memcpy_chk, __fprintf_chk).
shim=$(mktemp -d)
dl_stub=$(mktemp -d)
trap 'rm -rf "$shim" "$dl_stub"' EXIT
ar rcs "$dl_stub/libdl.a"
real_cargo=$(command -v cargo)
zigbuild=$(command -v cargo-zigbuild)
cat > "$shim/cargo" <<SHIMEOF
#!/usr/bin/env bash
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
chmod +x "$shim/cargo"
export PATH="$shim:$PATH"

# musl libc has dlopen built in, but libloading (via dioxus→subsecond) still
# emits #[link(name = "dl")]; the empty archive satisfies it.
export RUSTFLAGS="${RUSTFLAGS:-} -L $dl_stub"

echo "== building the daemon for x86_64-unknown-linux-musl (CI's target)"
(cd "$checkout/memvault/crates/memvault-web" && npm run tailwind:build)
(cd "$checkout/server" && dx build --package mac-mgmt --release --embed \
  @client --platform web --no-default-features --features web \
  @server --platform server --target x86_64-unknown-linux-musl \
    --features self-update,services,relay,memvault)

bin="$checkout/target/dx/mac-mgmt/release/web/server"
[ -x "$bin" ] || { echo "expected the musl binary at $bin" >&2; exit 1; }
echo
echo "== checking it is static"
if ldd "$bin" 2>&1 | grep -q "not a dynamic executable\|statically linked"; then
  echo "OK: static"
else
  echo "WARNING: still dynamically linked — instances will not run it:"
  ldd "$bin" 2>&1 | tail -4
fi

echo
echo "== publishing to the local xzar"
BIN="$bin" "$_here/publish-daemon.sh"
