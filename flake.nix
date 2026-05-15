{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    nixos2docker.url = "git+https://git.plan.ai/plan-ai/nixos2docker";
    nixos2docker.inputs.nixpkgs.follows = "nixpkgs";
    gitlab-incus-image.url = "git+https://git.mkg20001.io/mkg20001/gitlab-incus-image.git";
    gitlab-incus-image.inputs.nixpkgs.follows = "nixpkgs";
    xzar.url = "github:mkg20001/xzar";
    xzar.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, nixos2docker, gitlab-incus-image, xzar, ... }:
    {
      overlays.default = import ./overlay.nix { gitSha = self.rev or self.dirtyRev or "unknown"; };
      nixosModules.default = import ./server/module.nix;
      nixosModules.web-agency = import ./web-agency/server/module.nix;
      nixosModules.web-agency-proxy = import ./web-agency/proxy/module.nix;
      nixosModules.daemon = import ./daemon/module.nix;
      nixosModules.relay = import ./relay/module.nix;
      nixosModules.runner = import ./runner/module.nix;
      nixosModules.nix-driver-sync = import ./nix-driver-sync/module.nix;

      # NixOS-in-Docker test images
      # Build with:
      #   nix build .#nixosConfigurations.test-mac-mgmt-relay.config.system.build.dockerImage
      #   nix build .#nixosConfigurations.test-mac-mgmt-server.config.system.build.dockerImage
      #   nix build .#nixosConfigurations.test-mac-mgmt-daemon.config.system.build.dockerImage
      # Then: docker load < result
      # Run:  docker compose -f docker-compose.test.yml up

      nixosConfigurations = let
        x86Pkgs = import nixpkgs { system = "x86_64-linux"; };

        # Shared self-signed CA for all test containers
        sharedCA = x86Pkgs.runCommand "mac-mgmt-test-ca" {
          nativeBuildInputs = [ x86Pkgs.openssl ];
        } ''
          mkdir -p $out
          openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
            -keyout $out/ca-key.pem -out $out/ca-cert.pem \
            -days 3650 -nodes -subj "/CN=mac-mgmt Test CA"
        '';

        # Generate a TLS cert signed by the shared CA for a given service name
        mkServiceCert = name: x86Pkgs.runCommand "mac-mgmt-test-cert-${name}" {
          nativeBuildInputs = [ x86Pkgs.openssl ];
        } ''
          mkdir -p $out
          openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
            -keyout $out/key.pem -out $out/csr.pem \
            -nodes -subj "/CN=${name}"
          openssl x509 -req -in $out/csr.pem \
            -CA ${sharedCA}/ca-cert.pem -CAkey ${sharedCA}/ca-key.pem \
            -CAcreateserial -out $out/cert.pem -days 3650 \
            -extfile <(printf "subjectAltName=DNS:${name},DNS:localhost\nbasicConstraints=CA:FALSE")
          rm $out/csr.pem
        '';

        sharedCAModule = { ... }: {
          security.pki.certificateFiles = [ "${sharedCA}/ca-cert.pem" ];
          environment.etc."mac-mgmt-test-ca/ca-cert.pem".source = "${sharedCA}/ca-cert.pem";
          environment.etc."mac-mgmt-test-ca/ca-key.pem".source = "${sharedCA}/ca-key.pem";
        };

        # Pregenerated nix binary cache signing keys for the test xzar instance
        xzarSigningKey = "test-mac-mgmt-server:UhyyW66MgyTdhVQkOgo9Af6d7Pakohkx+rU+SgNYnEyCYSuae0VPG8/DnH0kAFnWI+afmzG1tZS5byWZYCU/cQ==";
        xzarPublicKey = "test-mac-mgmt-server:gmErmntFTxvPw5x9JABZ1iPmn5sxtbWUuW8lmWAlP3E=";

        # Nginx TLS termination module for a service
        mkTlsModule = { name, upstreamPort, listenPort ? 443 }: { ... }: let
          cert = mkServiceCert name;
        in {
          services.nginx = {
            enable = true;
            virtualHosts.${name} = {
              listenAddresses = [ "0.0.0.0" ];
              forceSSL = true;
              sslCertificate = "${cert}/cert.pem";
              sslCertificateKey = "${cert}/key.pem";
              locations."/" = {
                proxyPass = "http://127.0.0.1:${toString upstreamPort}";
                proxyWebsockets = true;
              };
            };
          };
          networking.firewall.allowedTCPPorts = [ listenPort ];
        };
      in {
        test-mac-mgmt-relay = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            nixos2docker.nixosModules.default
            self.nixosModules.relay
            sharedCAModule
            (mkTlsModule { name = "test-mac-mgmt-relay"; upstreamPort = 7380; })
            ({ lib, ... }: {
              nixpkgs.overlays = [ self.overlays.default ];

              virtualisation.dockerImage.name = "test-mac-mgmt-relay";
              virtualisation.dockerImage.tag = "latest";

              networking.hostName = "relay";

              services.mac-mgmt-relay = {
                enable = true;
                settings = {
                  listen_addr = "0.0.0.0:7380";
                  server_api_url = "https://test-mac-mgmt-server";
                  proxy_hostname = "test-mac-mgmt-relay";
                  proxy_url = "https://test-mac-mgmt-relay";
                  data_dir = "/var/lib/mac-mgmt-relay";
                  cors_origins = [ "https://test-mac-mgmt-server" ];
                };
              };

              # libp2p QUIC enumerates interfaces via netlink
              systemd.services.mac-mgmt-relay.serviceConfig.RestrictAddressFamilies =
                lib.mkForce [ "AF_INET" "AF_INET6" "AF_UNIX" "AF_NETLINK" ];

              fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
              boot.loader.grub.enable = false;
              system.stateVersion = "24.11";
            })
          ];
        };

        test-mac-mgmt-server = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            nixos2docker.nixosModules.default
            self.nixosModules.default
            xzar.nixosModules.xzar
            sharedCAModule
            (mkTlsModule { name = "test-mac-mgmt-server"; upstreamPort = 7378; })
            ({ ... }: {
              nixpkgs.overlays = [ (import rust-overlay) self.overlays.default xzar.overlays.default ];

              virtualisation.dockerImage.name = "test-mac-mgmt-server";
              virtualisation.dockerImage.tag = "latest";

              networking.hostName = "mac-mgmt-server";

              services.mac-mgmt-server = {
                enable = true;
                settings = {
                  api.external_url = "https://test-mac-mgmt-server/";
                  git.state_dir = "/var/lib/mac-mgmt-server";
                  xzar = {
                    url = "http://localhost:17788";
                    token = "test-token";
                    public_key = xzarPublicKey;
                  };
                };
              };

              services.xzar-server = {
                enable = true;
                config = {
                  signingKey = xzarSigningKey;
                  signingPubKey = xzarPublicKey;
                  externalUrl = "https://test-mac-mgmt-xzar";
                };
              };

              # Separate TLS vhost for xzar binary cache
              services.nginx.virtualHosts."test-mac-mgmt-xzar" = let
                cert = mkServiceCert "test-mac-mgmt-xzar";
              in {
                listenAddresses = [ "0.0.0.0" ];
                forceSSL = true;
                sslCertificate = "${cert}/cert.pem";
                sslCertificateKey = "${cert}/key.pem";
                locations."/" = {
                  proxyPass = "http://127.0.0.1:17788";
                  proxyWebsockets = true;
                  extraConfig = ''
                    client_max_body_size 10g;
                    proxy_request_buffering off;
                  '';
                };
              };

              fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
              boot.loader.grub.enable = false;
              system.stateVersion = "24.11";
            })
          ];
        };

        test-mac-mgmt-daemon = nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            nixos2docker.nixosModules.default
            self.nixosModules.daemon
            sharedCAModule
            ({ ... }: {
              nixpkgs.overlays = [ self.overlays.default ];

              virtualisation.dockerImage.name = "test-mac-mgmt-daemon";
              virtualisation.dockerImage.tag = "latest";

              networking.hostName = "daemon";

              services.mac-mgmt = {
                enable = true;
                version = "test";
                system = "x86_64-linux";
                serverUrl = "https://test-mac-mgmt-server";
                environmentFile = "/etc/mac-mgmt.env";
                settings = {
                  daemon.log_level = "info";
                };
              };

              nix.settings = {
                substituters = [ "https://test-mac-mgmt-xzar" ];
                trusted-public-keys = [ xzarPublicKey ];
              };

              fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
              boot.loader.grub.enable = false;
              system.stateVersion = "24.11";
            })
          ];
        };
      };
    } //
    flake-utils.lib.eachDefaultSystem (system:
      let
        gitSha = self.rev or self.dirtyRev or "unknown";
        overlays = [
          (import rust-overlay)
          (import ./overlay.nix { inherit gitSha; })
          xzar.overlays.default
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

        inherit (pkgs) mac-mgmt mac-mgmt-server mac-mgmt-server-mgmt mac-mgmt-server-skill-center mac-mgmt-server-skill-importer mac-mgmt-relay mac-mgmt-runner mac-mgmt-relay-ssh web-agency-server web-agency-proxy nix-driver-sync;
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
            xzar-client  # binary cache client

            # web-agency (cargo-progenitor installed via: cargo install cargo-progenitor)
            wrangler

            # Trainer fine-tuning (Python + CUDA/Vulkan)
            (python3.withPackages (ps: with ps; [
              torch
              transformers
              datasets
              peft
              trl
              bitsandbytes
              safetensors
              sentencepiece
              protobuf
              accelerate
              scipy
              unsloth
            ]))

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
          nix-driver-sync = nix-driver-sync;
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

            image = (nixpkgs.lib.nixosSystem {
              system = "x86_64-linux";
              modules = [
                "${nixpkgs}/nixos/modules/virtualisation/lxc-container.nix"
                gitlab-incus-image.nixosModules.gitlab-incus-image
                ({ pkgs, lib, ... }: {
                  environment.systemPackages = with pkgs; [
                    openssh
                    rsync
                    pkgs.xzar-client 
                    pixz
                  ];

                  nixpkgs.overlays = [
                    xzar.overlays.default
                  ];

                  programs.git.config.advice.detachedHead = false;

                  nix.settings = {
                    substituters = [
                      "https://xzar.plan.ai"
                    ];
                    trusted-public-keys = [
                      "xzar.plan.ai:KUE66pjr6UX5HHCn9kedN1DJ2J5nSlBrKmE7tUjXewE="
                    ];
                  };
                })
              ];
            }).config.system.build.gitlab-incus-image;
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
