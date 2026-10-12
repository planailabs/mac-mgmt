{
  lib,
  fetchurl,
  rustPlatform,
  pkg-config,
  openssl,
  dioxus-cli-patched,
  nodejs,
  wasm-bindgen-cli_0_2_121,
  binaryen,
  tailwindcss_3,
  lld,
  gitSha ? "unknown",
}:

let
  swagger-ui = fetchurl {
    url = "https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip";
    hash = "sha256-SBJE0IEgl7Efuu73n3HZQrFxYX+cn5UU5jrL4T5xzNw=";
  };
in

rustPlatform.buildRustPackage {
  pname = "mac-mgmt-server";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;
  cargoLock.outputHashes = import ../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "mac-mgmt-server" ];

  postPatch = ''
    cp -rL design design-canonical
    rm design
    mv design-canonical design
    # cargo resolves symlinks, so the vendored memvault/plan-ai-design (excluded
    # from memvault's workspace) canonicalises to the same directory the
    # workspace members reach as `design` -- "package collision in the lockfile".
    # It only exists because private/ and memvault/ are unpacked here; a plain
    # checkout has neither. Drop vendor-only copies, then repoint the two
    # vendored path deps at `design`.
    rm -rf memvault/plan-ai-design common/plan-ai-actions
    substituteInPlace server/package.json \
      --replace-fail '../memvault/plan-ai-design/assets/input.css' '../design/assets/input.css'
    substituteInPlace memvault/crates/memvault-web/Cargo.toml \
      --replace-fail 'path = "../../plan-ai-design"' 'path = "../../../design"'
  '';

  nativeBuildInputs = [
    pkg-config
    dioxus-cli-patched
    nodejs
    wasm-bindgen-cli_0_2_121
    binaryen
    tailwindcss_3
    lld
  ];

  buildInputs = [
    openssl
  ];

  SWAGGER_UI_DOWNLOAD_URL = "file://${swagger-ui}";
  env.GIT_SHA = gitSha;

  doCheck = false;

  # Build with dx instead of cargo so assets and WASM are bundled
  buildPhase = ''
    runHook preBuild

    # dx's cargo-metadata watchdog (~200s) is too short under parallel load --
    # cargo metadata waits on the shared package-cache lock while other builds
    # run, so dx times out before Cargo reports anything (same trap already
    # handled in overlay.nix). Resolve metadata up front so the real error, if
    # any, is Cargo's.
    timeout 600 cargo metadata --format-version=1 >/dev/null

    # Tailwind CSS
    pushd server
    npm run tailwind:build
    popd

    dx build --release --fullstack --package mac-mgmt-server

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin $out/share
    cp -r target/dx/mac-mgmt-server/release/web $out/share/mac-mgmt-server
    if [ -e $out/share/mac-mgmt-server/mac-mgmt-server ]; then
      ln -s $out/share/mac-mgmt-server/mac-mgmt-server $out/bin/mac-mgmt-server
    else
      ln -s $out/share/mac-mgmt-server/server $out/bin/mac-mgmt-server
    fi

    runHook postInstall
  '';

  meta = {
    description = "Mac management server with web UI";
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-server";
  };
}
