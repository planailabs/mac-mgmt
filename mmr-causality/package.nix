{
  lib,
  rustPlatform,
  gitSha ? "unknown",
}:

# Builds both binaries of the mmr-causality crate: `mmrc` (the chaos CLI) and
# `mmrcd` (the incus orchestration daemon). No openssl — reqwest uses rustls.
rustPlatform.buildRustPackage {
  pname = "mmr-causality";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;
  cargoLock.outputHashes = import ../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "mmr-causality" ];
  env.GIT_SHA = gitSha;

  postPatch = ''
    cp -rL design design-canonical
    rm design
    mv design-canonical design
    substituteInPlace memvault/crates/memvault-web/Cargo.toml \
      --replace-fail 'path = "../../plan-ai-design"' 'path = "../../../design"'
  '';

  doCheck = false;

  meta = {
    description = "mmrc chaos CLI + mmrcd incus orchestration daemon";
    license = lib.licenses.asl20;
    mainProgram = "mmrcd";
  };
}
