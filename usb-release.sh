#!/usr/bin/env bash
#
# Build the sovereign-AI USB binary (`mac-mgmt --features usb`, static musl, with
# the embedded web UI) for each supported Linux arch, package it into a
# per-arch tarball with a SHA-256, upload to the GitLab generic package
# registry, and publish a GitLab Release whose assets the in-binary updater
# (daemon/src/usb_update.rs) consumes.
#
# Asset naming matches usb_update::asset_name(): mac-mgmt-usb-<arch>.tar.gz
# Tag is `v<version>` so usb_update::is_newer() parses it cleanly.
#
# Runs in CI (deploy stage, trunk) inside `nix develop`. Requires the GitLab CI
# environment (CI_API_V4_URL, CI_PROJECT_ID, CI_JOB_TOKEN, CI_COMMIT_SHA).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export ENVIRONMENT=production

VERSION="$(grep '^version' "$SCRIPT_DIR/daemon/Cargo.toml" | head -1 | cut -d'"' -f2)"
NIXPKGS_REV="$(grep -A4 '"nixpkgs"' "$SCRIPT_DIR/flake.lock" | grep '"rev"' | head -1 | cut -d'"' -f4)"
TAG="v${VERSION}"
OUT="$SCRIPT_DIR/usb-release"
rm -rf "$OUT"
mkdir -p "$OUT"

echo "Building USB release ${VERSION} (nixpkgs ${NIXPKGS_REV})"

# ── Tailwind CSS (the embedded web UI needs it built) ───────────────────
(cd "$SCRIPT_DIR/memvault/crates/memvault-web" && npm run tailwind:build)

# ── Cross-compilation cargo shim (cargo-zigbuild for musl targets) ──────
# Mirrors xzar.sh: route *linux-musl* targets through cargo-zigbuild so C build
# scripts + the final linker use the musl C runtime, not host glibc.
CARGO_SHIM="$(mktemp -d)"
REAL_CARGO="$(which cargo)"
ZIGBUILD="$(which cargo-zigbuild)"
cat > "$CARGO_SHIM/cargo" <<SHIM
#!/usr/bin/env bash
use_zig=false
prev=""
args=()
for arg in "\$@"; do
  case "\$prev" in
    --target) [[ "\$arg" == *linux-musl* ]] && use_zig=true ;;
  esac
  case "\$arg" in
    --target=*linux-musl*) use_zig=true ;;
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
cleanup() { export PATH="${PATH#"$CARGO_SHIM:"}"; rm -rf "$CARGO_SHIM"; }
trap cleanup EXIT

# musl libc has dlopen/dlsym built in — provide an empty libdl.a stub so the
# linker resolves -ldl emitted by libloading (via dioxus→subsecond).
DL_STUB="$(mktemp -d)"
ar rcs "$DL_STUB/libdl.a"
export RUSTFLAGS="${RUSTFLAGS:-} -L $DL_STUB"

build_arch() {
  local rust_target="$1" arch="$2"
  echo "── building $arch ($rust_target) ──"
  dx build --package mac-mgmt --release --embed \
    @client --platform web --no-default-features --features web \
    @server --platform server --target "$rust_target" --features usb

  local bin_src="$SCRIPT_DIR/target/dx/mac-mgmt/release/web/server"
  [ -f "$bin_src" ] || { echo "missing built binary: $bin_src" >&2; exit 1; }

  local stage
  stage="$(mktemp -d)"
  mkdir -p "$stage/bin"
  cp "$bin_src" "$stage/bin/mac-mgmt"
  chmod +x "$stage/bin/mac-mgmt"
  echo "$NIXPKGS_REV" > "$stage/nixpkgs-rev"

  local tarball="mac-mgmt-usb-${arch}.tar.gz"
  tar -C "$stage" -czf "$OUT/$tarball" .
  ( cd "$OUT" && sha256sum "$tarball" > "${tarball}.sha256" )
  rm -rf "$stage"
  echo "packaged $OUT/$tarball"
}

build_arch x86_64-unknown-linux-musl x86_64-linux
build_arch aarch64-unknown-linux-musl aarch64-linux

rm -rf "$DL_STUB"
unset RUSTFLAGS

# ── Publish to GitLab (generic package registry + Release) ──────────────
if [ -z "${CI_API_V4_URL:-}" ]; then
  echo "not in CI — built tarballs in $OUT, skipping upload"
  exit 0
fi

PKG="mac-mgmt-usb"
declare -a LINKS
for f in "$OUT"/*; do
  name="$(basename "$f")"
  url="${CI_API_V4_URL}/projects/${CI_PROJECT_ID}/packages/generic/${PKG}/${VERSION}/${name}"
  echo "uploading $name"
  curl --fail --silent --show-error \
    --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
    --upload-file "$f" "$url"
  LINKS+=("{\"name\":\"${name}\",\"url\":\"${url}\"}")
done

links_json="$(IFS=,; echo "${LINKS[*]}")"
echo "creating release ${TAG}"
# Best-effort: delete an existing same-tag release so re-runs replace assets.
curl --silent --request DELETE \
  --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
  "${CI_API_V4_URL}/projects/${CI_PROJECT_ID}/releases/${TAG}" >/dev/null || true

curl --fail --silent --show-error --request POST \
  --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
  --header "Content-Type: application/json" \
  --data "{
    \"name\": \"USB ${VERSION}\",
    \"tag_name\": \"${TAG}\",
    \"ref\": \"${CI_COMMIT_SHA}\",
    \"description\": \"Sovereign-AI USB build ${VERSION} (nixpkgs ${NIXPKGS_REV}).\",
    \"assets\": { \"links\": [${links_json}] }
  }" \
  "${CI_API_V4_URL}/projects/${CI_PROJECT_ID}/releases" >/dev/null

echo "USB release ${TAG} published"
