{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    {
      overlays.default = import ./overlay.nix { gitSha = self.rev or self.dirtyRev or "unknown"; };
      nixosModules.default = import ./server/module.nix;
      nixosModules.web-agency = import ./web-agency/server/module.nix;
      nixosModules.web-agency-proxy = import ./web-agency/proxy/module.nix;
      nixosModules.daemon = import ./daemon/module.nix;
      nixosModules.relay = import ./relay/module.nix;
      nixosModules.runner = import ./runner/module.nix;
    } //
    flake-utils.lib.eachDefaultSystem (system:
      let
        gitSha = self.rev or self.dirtyRev or "unknown";
        overlays = [
          (import rust-overlay)
          (import ./overlay.nix { inherit gitSha; })
        ];
        pkgs = import nixpkgs { inherit system overlays; };
        toolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [
            "aarch64-apple-darwin"
            "x86_64-apple-darwin"
            "x86_64-unknown-linux-musl"
            "wasm32-unknown-unknown"
          ];
        };

        darwinDeps = pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
        ];

        inherit (pkgs) mac-mgmt mac-mgmt-server mac-mgmt-server-mgmt mac-mgmt-server-skill-center mac-mgmt-server-skill-importer mac-mgmt-relay mac-mgmt-runner mac-mgmt-relay-ssh web-agency-server web-agency-proxy;
        relay-ssh = mac-mgmt-relay-ssh;

        # Standalone unpacked MacOSX SDK so cargo-zigbuild can satisfy
        # `-framework CoreFoundation` etc when cross-compiling Apple targets
        # from Linux. We pull the .src out of nixpkgs' darwin.apple_sdk_25
        # (a plain fetchurl FOD) and extract it with a Linux runCommand —
        # this avoids needing to build any darwin stdenv on the host.
        macosx-sdk = let
          darwinPkgs = import nixpkgs { system = "aarch64-darwin"; };
          sdkSrc = darwinPkgs.apple-sdk_26.src;
        in sdkSrc;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            toolchain
            cargo-edit
            cargo-zigbuild
            cargo-flamegraph
            zig
            rsync

            # Dioxus CLI
            dioxus-cli

            # Build dependencies
            pkg-config
            openssl
            nodejs
            tailwindcss_3

            # Dev tools
            overmind
            cargo-watch
            xz  # for nixpkgs archive generation

            # web-agency (cargo-progenitor installed via: cargo install cargo-progenitor)
            wrangler

            # web-agency-proxy (BoringSSL build via boring-sys)
            cmake
            clang
            libclang.lib

            # For WASM
            wasm-pack
            wasm-bindgen-cli_0_2_114
            binaryen  # wasm-opt

          ] ++ darwinDeps;

          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        };

        packages = {
          default = mac-mgmt;
          server = mac-mgmt-server;
          server-mgmt = mac-mgmt-server-mgmt;
          server-skill-center = mac-mgmt-server-skill-center;
          server-skill-importer = mac-mgmt-server-skill-importer;
          relay = mac-mgmt-relay;
          runner = mac-mgmt-runner;
          relay-ssh = relay-ssh;
          web-agency = web-agency-server;
          web-agency-proxy = web-agency-proxy;
          macosx-sdk = macosx-sdk;
        } // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux (
          let images = import ./docker.nix {
            inherit pkgs mac-mgmt-server mac-mgmt-relay mac-mgmt-runner;
            mac-mgmt-relay-ssh = relay-ssh;
            tag = gitSha;
          };
          in {
            docker-server = images.server;
            docker-relay = images.relay;
            docker-runner = images.runner;
            docker-relay-ssh = images.relay-ssh;
          }
        ) // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
          tarball = pkgs.runCommand "mac-mgmt-tarball" {} ''
            mkdir -p $out pack
            cp ${mac-mgmt}/bin/mac-mgmt pack/mac-mgmt
            cd pack
            tar czf $out/mac-mgmt.tar.gz mac-mgmt
          '';
        };

        checks = pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          relay-integration = pkgs.callPackage ./tests/relay.nix {
            inherit mac-mgmt-relay;
          };
          metrics-federation = pkgs.callPackage ./tests/metrics.nix {
            inherit mac-mgmt-relay;
          };
          sse-push = pkgs.callPackage ./tests/sse-push.nix { };
          sse-daemon = pkgs.callPackage ./tests/sse-daemon.nix { };

          # Deterministic simulation tests — mock server + real daemon code.
          # Fast (seconds) compared to VM-based tests above (minutes).
          /* sim-tests = pkgs.rustPlatform.buildRustPackage {
            pname = "mac-mgmt-sim-tests";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoLock.outputHashes = import ./extra-hashes.nix;
            cargoBuildFlags = [ "-p" "sim-tests" ];
            doCheck = true;
            cargoTestFlags = [ "-p" "sim-tests" ];
            env.GIT_SHA = gitSha;
          }; */
        };
      });
}
