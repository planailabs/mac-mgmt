{
  lib,
  rustPlatform,
  openssl,
  pkg-config,
}:

rustPlatform.buildRustPackage {
  pname = "relay-ssh";
  version = "0.1.0";
  src = ./..;
  cargoLock.lockFile = ../Cargo.lock;
  cargoLock.outputHashes = import ../extra-hashes.nix;

  cargoBuildFlags = [ "-p" "relay-ssh" ];

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
    description = "SSH CLI tool for mac-mgmt relay";
    license = lib.licenses.asl20;
    mainProgram = "relay-ssh";
  };
}
