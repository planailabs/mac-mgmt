{
  lib,
  fetchurl,
  rustPlatform,
  pkg-config,
  openssl,
  dioxus-cli,
  nodejs,
  wasm-bindgen-cli_0_2_114,
  binaryen,
  tailwindcss_3,
  lld,
  gitSha ? "unknown",
  # Server-side feature flags. Empty string = default features (monolith).
  # When set, builds client/server separately to avoid the Dioxus fullstack
  # SSR WASM issue (server deps like tokio/mio can't compile to WASM).
  serverFeatures ? "",
  pnameSuffix ? "",
  description ? "Mac management server with web UI",
}:

let
  swagger-ui = fetchurl {
    url = "https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip";
    hash = "sha256-SBJE0IEgl7Efuu73n3HZQrFxYX+cn5UU5jrL4T5xzNw=";
  };
  isSplit = serverFeatures != "";
in

rustPlatform.buildRustPackage {
  pname = "mac-mgmt-server${pnameSuffix}";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;
  cargoLock.outputHashes = import ../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "mac-mgmt-server" ];

  nativeBuildInputs = [
    pkg-config
    dioxus-cli
    nodejs
    wasm-bindgen-cli_0_2_114
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

  # Build with dx instead of cargo so assets and WASM are bundled.
  # For split feature builds, we build client (WASM) and server separately
  # because Dioxus fullstack mode does an internal WASM build of the server
  # for SSR, which fails when server deps (tokio/mio) can't target WASM.
  buildPhase = ''
    runHook preBuild

    # Tailwind CSS
    pushd server
    npm run tailwind:build
    popd

  '' + (if isSplit then ''
    # Split build: client WASM with web-only features, server native with full features
    dx build --release --platform web --package mac-mgmt-server --no-default-features --features "web,webui"
    cargo build --release -p mac-mgmt-server --no-default-features --features "${serverFeatures}"
  '' else ''
    # Monolith: dx fullstack handles feature splitting automatically
    dx build --release --fullstack --package mac-mgmt-server
  '') + ''

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin $out/share/mac-mgmt-server
  '' + (if isSplit then ''
    # Split build: assemble from separate outputs
    # WASM client assets from dx
    cp -r target/dx/mac-mgmt-server/release/web/public $out/share/mac-mgmt-server/public
    # Server binary from cargo
    cp target/release/mac-mgmt-server $out/share/mac-mgmt-server/mac-mgmt-server || \
      cp target/release/server $out/share/mac-mgmt-server/mac-mgmt-server
    ln -s $out/share/mac-mgmt-server/mac-mgmt-server $out/bin/mac-mgmt-server
  '' else ''
    # Monolith: dx fullstack output
    cp -r target/dx/mac-mgmt-server/release/web/* $out/share/mac-mgmt-server/
    if [ -e $out/share/mac-mgmt-server/mac-mgmt-server ]; then
      ln -s $out/share/mac-mgmt-server/mac-mgmt-server $out/bin/mac-mgmt-server
    else
      ln -s $out/share/mac-mgmt-server/server $out/bin/mac-mgmt-server
    fi
  '') + ''

    runHook postInstall
  '';

  meta = {
    inherit description;
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-server";
  };
}
