#!/bin/sh
# Build the daemon with the flake's own package — the CI release path.
#
# NOT enough on its own for a fleet instance: the flake builds for the host
# (cargo-auditable, `--offline`), so the binary is a glibc build whose ELF
# interpreter is a /nix/store path. An instance has a plain Ubuntu userland
# and no /nix/store, so it dies with "not found". CI publishes the runnable
# artifact by cross-building to x86_64-unknown-linux-musl in xzar.sh (with
# cargo-zigbuild and a libdl.a stub) and uploading that to the binary cache.
#
# Use this for a quick local build; use CI's artifact — or the musl route in
# this directory — for anything an instance has to run.
set -eu
. "$(dirname "$0")/lib.sh"

cd "$checkout"
out=$(nix build --no-link --print-out-paths ".#default")

echo "built: $out/bin/mac-mgmt"
echo
echo "linked against:"
ldd "$out/bin/mac-mgmt" 2>&1 | tail -3
echo
case "$(ldd "$out/bin/mac-mgmt" 2>/dev/null)" in
  *not\ a\ dynamic\ executable*|*"statically linked"*)
    echo "OK: static — this one can run on a fleet instance."
    ;;
  *)
    echo "WARNING: dynamically linked, interpreter is a /nix/store path."
    echo "         Fleet instances cannot run this. See the header of this script."
    ;;
esac
