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
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        toolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [
            "aarch64-apple-darwin"
            "x86_64-apple-darwin"
          ];
        };

        darwinDeps = pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          pkgs.libiconv
        ];

        mac-mgmt = pkgs.rustPlatform.buildRustPackage {
          pname = "mac-mgmt";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildInputs = darwinDeps;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            toolchain
            pkgs.cargo-edit
          ] ++ darwinDeps;

          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
        };

        packages.default = mac-mgmt;
      } // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
        packages.tarball = pkgs.runCommand "mac-mgmt-tarball" {} ''
          mkdir -p $out pack
          cp ${mac-mgmt}/bin/mac-mgmt pack/mac-mgmt
          cd pack
          tar czf $out/mac-mgmt.tar.gz mac-mgmt
        '';
      });
}
