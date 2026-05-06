{ gitSha ? "unknown" }:

final: prev:
let
  # Thin wrapper that re-exports the monolith server binary with a
  # MAC_MGMT_SERVER_MODE env var preset, so only the selected API
  # route group is mounted at runtime.  The heavy build happens once
  # in mac-mgmt-server; these just write a small shell script.
  mkServerMode = { mode, pnameSuffix, description }:
    prev.runCommand "mac-mgmt-server${pnameSuffix}" {} ''
      mkdir -p $out/bin $out/share
      ln -s ${final.mac-mgmt-server}/share/mac-mgmt-server $out/share/mac-mgmt-server
      cat > $out/bin/mac-mgmt-server${pnameSuffix} <<'WRAPPER'
      #!/bin/sh
      export MAC_MGMT_SERVER_MODE="${mode}"
      exec "${final.mac-mgmt-server}/bin/mac-mgmt-server" "$@"
      WRAPPER
      chmod +x $out/bin/mac-mgmt-server${pnameSuffix}
    '';
in
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

  mac-mgmt-server-mgmt = mkServerMode {
    mode = "mgmt";
    pnameSuffix = "-mgmt";
    description = "Mac management server (mgmt only)";
  };

  mac-mgmt-server-skill-center = mkServerMode {
    mode = "skill-center";
    pnameSuffix = "-skill-center";
    description = "Mac management server (skill center only)";
  };

  mac-mgmt-server-skill-importer = mkServerMode {
    mode = "skill-importer";
    pnameSuffix = "-skill-importer";
    description = "Mac management server (skill importer only)";
  };

  web-agency-server = prev.callPackage ./web-agency/server/package.nix { inherit gitSha; };
  web-agency-proxy = prev.callPackage ./web-agency/proxy/package.nix { };

  mac-mgmt-relay = prev.callPackage ./relay/package.nix { };
  mac-mgmt-runner = prev.callPackage ./runner/package.nix { inherit gitSha; };
  mac-mgmt-relay-ssh = prev.callPackage ./relay-ssh/package.nix { };
}
