{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
  dioxus-cli-patched,
  nodejs,
  wasm-bindgen-cli_0_2_121,
  binaryen,
  tailwindcss_3,
  lld,
  wrangler,
  makeWrapper,
  gitSha ? "unknown",
}:

rustPlatform.buildRustPackage {
  pname = "web-agency-server";
  version = "0.1.0";
  src = ../..;
  cargoLock.lockFile = ../../Cargo.lock;
  cargoLock.outputHashes = import ../../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "web-agency-server" ];

  nativeBuildInputs = [
    pkg-config
    dioxus-cli-patched
    nodejs
    wasm-bindgen-cli_0_2_121
    binaryen
    tailwindcss_3
    lld
    makeWrapper
  ];

  buildInputs = [
    openssl
  ];

  env.GIT_SHA = gitSha;

  doCheck = false;

  # Build with dx instead of cargo so assets and WASM are bundled
  buildPhase = ''
    runHook preBuild

    # Tailwind CSS
    pushd web-agency/server
    npm run tailwind:build
    popd

    dx build --release --fullstack --package web-agency-server

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin $out/share
    cp -r target/dx/web-agency-server/release/web $out/share/web-agency-server

    # Wrap the binary so wrangler is on PATH (needed for Pages deployments)
    if [ -e $out/share/web-agency-server/web-agency-server ]; then
      makeWrapper $out/share/web-agency-server/web-agency-server $out/bin/web-agency-server \
        --prefix PATH : ${lib.makeBinPath [ wrangler ]}
    else
      makeWrapper $out/share/web-agency-server/server $out/bin/web-agency-server \
        --prefix PATH : ${lib.makeBinPath [ wrangler ]}
    fi

    runHook postInstall
  '';

  meta = {
    description = "Web agency server – domain, DNS, and Cloudflare Pages management";
    license = lib.licenses.asl20;
    mainProgram = "web-agency-server";
  };
}
