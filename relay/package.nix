{
  lib,
  rustPlatform,
  openssl,
  pkg-config,
}:

rustPlatform.buildRustPackage {
  pname = "mac-mgmt-relay";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;
  cargoLock.outputHashes = import ../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "mac-mgmt-relay" ];

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
    description = "SSH relay for mac-mgmt daemons";
    license = lib.licenses.asl20;
    mainProgram = "mac-mgmt-relay";
  };
}
