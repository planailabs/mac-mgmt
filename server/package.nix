{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
  dioxus-cli,
  nodejs,
  wasm-bindgen-cli_0_2_114,
  binaryen,
  tailwindcss,
}:

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
    tailwindcss
  ];

  buildInputs = [
    openssl
  ];

  # Build with dx instead of cargo so assets and WASM are bundled
  buildPhase = ''
    runHook preBuild

    # Tailwind CSS
    cd server
    npx tailwindcss -i input.css -o public/tailwind.css --minify
    cd ..

    dx build --release --platform fullstack

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin
    cp -r target/dx/mac-mgmt-server/release/web $out/share/mac-mgmt-server
    cp target/dx/mac-mgmt-server/release/server $out/bin/mac-mgmt-server

    runHook postInstall
  '';

  meta = {
    description = "Mac management server with web UI";
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-server";
  };
}
