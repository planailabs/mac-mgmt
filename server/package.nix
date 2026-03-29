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

  doCheck = false;

  # Build with dx instead of cargo so assets and WASM are bundled
  buildPhase = ''
    runHook preBuild

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
    ln -s $out/share/mac-mgmt-server/mac-mgmt-server $out/bin/mac-mgmt-server

    runHook postInstall
  '';

  meta = {
    description = "Mac management server with web UI";
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-server";
  };
}
