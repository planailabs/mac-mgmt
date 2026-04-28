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

  mac-mgmt-server-mgmt = prev.callPackage ./server/package.nix {
    inherit gitSha;
    pnameSuffix = "-mgmt";
    featureFlags = "server,webui,web,mgmt";
    description = "Mac management server (mgmt only) with web UI";
  };

  mac-mgmt-server-skill-center = prev.callPackage ./server/package.nix {
    inherit gitSha;
    pnameSuffix = "-skill-center";
    featureFlags = "server,webui,web,skill-center";
    description = "Mac management server (skill center only) with web UI";
  };

  mac-mgmt-server-skill-importer = prev.callPackage ./server/package.nix {
    inherit gitSha;
    pnameSuffix = "-skill-importer";
    featureFlags = "server,webui,web,skill-center,skill-importer";
    description = "Mac management server (skill importer only) with web UI";
  };

  mac-mgmt-relay = prev.callPackage ./relay/package.nix { };
  mac-mgmt-runner = prev.callPackage ./runner/package.nix { inherit gitSha; };
  mac-mgmt-relay-ssh = prev.callPackage ./relay-ssh/package.nix { };
}
