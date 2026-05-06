{
  lib,
  rustPlatform,
  cmake,
  clang,
  libclang,
  perl,
  git,
}:

rustPlatform.buildRustPackage {
  pname = "web-agency-proxy";
  version = "0.1.0";
  src = ../..;
  cargoLock.lockFile = ../../Cargo.lock;
  cargoLock.outputHashes = import ../../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "web-agency-proxy" ];

  nativeBuildInputs = [
    cmake   # boring-sys builds BoringSSL from source
    clang   # C/C++ compiler for BoringSSL
    perl    # BoringSSL build scripts need perl
    git     # boring-sys runs `git init` to apply patches to vendored BoringSSL
  ];

  env.LIBCLANG_PATH = "${libclang.lib}/lib";

  doCheck = false;

  meta = {
    description = "Web agency reverse proxy – Pingora-based TLS termination and routing";
    license = lib.licenses.asl20;
    mainProgram = "web-agency-proxy";
  };
}
