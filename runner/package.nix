{
  lib,
  rustPlatform,
  openssl,
  pkg-config,
}:

rustPlatform.buildRustPackage {
  pname = "mac-mgmt-runner";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;

  cargoBuildFlags = [ "-p" "mac-mgmt-runner" ];

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl ];

  doCheck = false;

  meta = {
    description = "Fleet orchestrator that spawns a matrix of mac-mgmt daemons in Incus";
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-runner";
  };
}
