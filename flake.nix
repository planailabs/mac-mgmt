{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, ... }:
    {
      nixosModules.default = import ./server/module.nix;
    } //
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
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

        mac-mgmt = pkgs.rustPlatform.buildRustPackage {
          pname = "mac-mgmt";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildInputs = darwinDeps;
        };

        mac-mgmt-server = pkgs.callPackage ./server/package.nix { };
        mac-mgmt-relay = pkgs.callPackage ./relay/package.nix { };
        relay-ssh = pkgs.callPackage ./relay-ssh/package.nix { };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            toolchain
            cargo-edit
            cargo-zigbuild
            zig
            rsync

            # Dioxus CLI
            dioxus-cli

            # Build dependencies
            pkg-config
            openssl
            nodejs

            # For WASM
            wasm-pack
            wasm-bindgen-cli_0_2_114
            binaryen  # wasm-opt

          ] ++ darwinDeps;

          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
        };

        packages.default = mac-mgmt;
        packages.server = mac-mgmt-server;
        packages.relay = mac-mgmt-relay;
        packages.relay-ssh = relay-ssh;

        checks = pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          relay-integration = pkgs.callPackage ./tests/relay.nix {
            inherit mac-mgmt-relay;
          };
        };
      } // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
        packages.tarball = pkgs.runCommand "mac-mgmt-tarball" {} ''
          mkdir -p $out pack
          cp ${mac-mgmt}/bin/mac-mgmt pack/mac-mgmt
          cd pack
          tar czf $out/mac-mgmt.tar.gz mac-mgmt
        '';
      });
}
