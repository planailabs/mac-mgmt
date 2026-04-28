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
  # Override to build a feature-subset variant.
  # Empty string = default features (monolith).
  featureFlags ? "",
  pnameSuffix ? "",
  description ? "Mac management server with web UI",
}:

let
  swagger-ui = fetchurl {
    url = "https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip";
    hash = "sha256-SBJE0IEgl7Efuu73n3HZQrFxYX+cn5UU5jrL4T5xzNw=";
  };
  dxFeatureArgs =
    if featureFlags == "" then ""
    else "--no-default-features --features \"${featureFlags}\"";
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

  # Build with dx instead of cargo so assets and WASM are bundled
  buildPhase = ''
    runHook preBuild

    # Tailwind CSS
    pushd server
    npm run tailwind:build
    popd

    dx build --release --fullstack --package mac-mgmt-server ${dxFeatureArgs}

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
    inherit description;
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-server";
  };
}
