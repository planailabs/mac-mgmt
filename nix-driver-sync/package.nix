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
    substituteInPlace memvault/crates/memvault-web/Cargo.toml \
      --replace-fail 'path = "../../plan-ai-design"' 'path = "../../../design"'
  '';

  doCheck = false;

  meta = {
    description = "Sync NixOS opengl-driver into Incus containers";
    license = lib.licenses.asl20;
    mainProgram = "nix-driver-sync";
  };
}
