{
  lib,
  rustPlatform,
  openssl,
  pkg-config,
  gitSha ? "unknown",
}:

rustPlatform.buildRustPackage {
  pname = "mac-mgmt-runner";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;
  cargoLock.outputHashes = import ../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "mac-mgmt-runner" ];
  env.GIT_SHA = gitSha;

  postPatch = ''
    cp -rL design design-canonical
    rm design
    mv design-canonical design
    substituteInPlace memvault/crates/memvault-web/Cargo.toml \
      --replace-fail 'path = "../../plan-ai-design"' 'path = "../../../design"'
  '';

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl ];

  doCheck = false;

  meta = {
    description = "Fleet orchestrator that spawns a matrix of mac-mgmt daemons in Incus";
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-runner";
  };
}
