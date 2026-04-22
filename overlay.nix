{ gitSha ? "unknown" }:

final: prev:
{
  mac-mgmt = prev.rustPlatform.buildRustPackage {
    pname = "mac-mgmt";
    version = "0.1.0";
    src = ./.;
    cargoLock.lockFile = ./Cargo.lock;
    cargoLock.outputHashes = import ./extra-hashes.nix;
    buildInputs = prev.lib.optionals prev.stdenv.isDarwin [ prev.libiconv ];
    env.GIT_SHA = gitSha;
  };

  mac-mgmt-server = prev.callPackage ./server/package.nix { inherit gitSha; };
  mac-mgmt-relay = prev.callPackage ./relay/package.nix { };
  mac-mgmt-runner = prev.callPackage ./runner/package.nix { inherit gitSha; };
  mac-mgmt-relay-ssh = prev.callPackage ./relay-ssh/package.nix { };
}
