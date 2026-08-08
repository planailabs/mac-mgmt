{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "nix-driver-sync";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;
  cargoLock.outputHashes = import ../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "nix-driver-sync" ];

  postPatch = ''
    cp -rL design design-canonical
    rm design
    mv design-canonical design
    rm -rf memvault/plan-ai-design
    ln -s ../../design memvault/plan-ai-design
  '';

  doCheck = false;

  meta = {
    description = "Sync NixOS opengl-driver into Incus containers";
    license = lib.licenses.asl20;
    mainProgram = "nix-driver-sync";
  };
}
