{
  inputs = {
    # Include git submodules (e.g. memvault in the flake source.
    self.submodules = true;

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
      nixosModules.mmrcd = import ./mmr-causality/module.nix;
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
            -CAserial $out/ca.srl -CAcreateserial -out $out/cert.pem -days 3650 \
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

        # Nginx TLS reverse proxy vhost for a named upstream
        mkTlsVhost = { name, upstreamPort, locationExtraConfig ? "" }: let
          cert = mkServiceCert name;
        in {
          # Listen on both IPv4 and IPv6 so the service is reachable over the
          # instance's public IPv6 (mmrc addresses instances by global IPv6).
          listenAddresses = [ "0.0.0.0" "[::]" ];
          forceSSL = true;
          sslCertificate = "${cert}/cert.pem";
          sslCertificateKey = "${cert}/key.pem";
          locations."/" = {
            proxyPass = "http://127.0.0.1:${toString upstreamPort}";
            proxyWebsockets = true;
          } // nixpkgs.lib.optionalAttrs (locationExtraConfig != "") {
            extraConfig = locationExtraConfig;
          };
        };

        # Module that enables nginx with one or more TLS vhosts and opens port 443
        mkTlsModule = vhosts: { ... }: {
          services.nginx = {
            enable = true;
            virtualHosts = nixpkgs.lib.listToAttrs (map (v: {
              name = v.name;
              value = mkTlsVhost v;
            }) vhosts);
          };
          networking.firewall.allowedTCPPorts = [ 443 ];
        };

        # Common base for all NixOS-in-Docker test containers
        baseModule = name: { ... }: {
          virtualisation.dockerImage.name = name;
          virtualisation.dockerImage.tag = "latest";
          fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
          boot.loader.grub.enable = false;
          system.stateVersion = "26.11";
        };

        mkTestSystem = { name, modules }: nixpkgs.lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            nixos2docker.nixosModules.default
            sharedCAModule
            (baseModule name)
          ] ++ modules;
        };
      in {
        test-mac-mgmt-relay = mkTestSystem {
          name = "test-mac-mgmt-relay";
          modules = [
            self.nixosModules.relay
            (mkTlsModule [{ name = "test-mac-mgmt-relay"; upstreamPort = 7380; }])
            ({ lib, ... }: {
              nixpkgs.overlays = [ self.overlays.default ];
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
            })
          ];
        };

        test-mac-mgmt-server = mkTestSystem {
          name = "test-mac-mgmt-server";
          modules = [
            self.nixosModules.default
            xzar.nixosModules.xzar
            (mkTlsModule [
              { name = "test-mac-mgmt-server"; upstreamPort = 7378; }
              { name = "test-mac-mgmt-xzar"; upstreamPort = 17788;
                locationExtraConfig = "client_max_body_size 10g;\nproxy_request_buffering off;"; }
            ])
            ({ ... }: let
              pkgs = nixpkgs.legacyPackages.x86_64-linux;
            in {
              nixpkgs.overlays = [ (import rust-overlay) self.overlays.default xzar.overlays.default ];
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
                  # Enable the chaos-node registration API in the antithesis
                  # test cluster only. Never set in production.
                  chaos.enabled = true;
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

              # Seed admin and federation tokens after the server has started
              # (migrations run on server startup). Token values are "admin" and
              # "federation", stored as sha256 hashes.
              systemd.services.mac-mgmt-seed-tokens = {
                description = "Seed test API tokens";
                after = [ "mac-mgmt.service" ];
                requires = [ "mac-mgmt.service" ];
                wantedBy = [ "multi-user.target" ];
                serviceConfig = {
                  Type = "oneshot";
                  RemainAfterExit = true;
                  User = "mac-mgmt";
                };
                script = ''
                  # Wait for the server to be ready
                  for i in $(seq 1 30); do
                    ${pkgs.curl}/bin/curl -sf http://localhost:7378/api/health && break
                    sleep 1
                  done

                  ${pkgs.postgresql}/bin/psql "postgres:///mac-mgmt?host=/run/postgresql" <<'SQL'
                    INSERT INTO tokens (id, cluster_id, token_hash, label, kind)
                    VALUES
                      (gen_random_uuid(), NULL,
                       '8c6976e5b5410415bde908bd4dee15dfb167a9c873fc4bb8a81f6f2ab448a918',
                       'test-admin', 'admin'),
                      (gen_random_uuid(), NULL,
                       '8ce333c5811acbdc46a0ba06bc27621a23b2550b1469b3b679a9f4a3c7470772',
                       'test-federation', 'federation')
                    ON CONFLICT DO NOTHING;
                  SQL
                '';
              };
            })
          ];
        };

        test-mac-mgmt-daemon = mkTestSystem {
          name = "test-mac-mgmt-daemon";
          modules = [
            self.nixosModules.daemon
            ({ ... }: {
              nixpkgs.overlays = [ self.overlays.default ];
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
            "aarch64-unknown-linux-musl"
            "x86_64-unknown-linux-musl"
            "wasm32-unknown-unknown"
            # memvault-extract builds WASI guest modules (OCR/audio/text/pdf)
            # via a build.rs that cross-compiles to wasm32-wasip1.
            "wasm32-wasip1"
          ];
        };

        darwinDeps = pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
        ];

        inherit (pkgs) mac-mgmt mac-mgmt-server mac-mgmt-server-mgmt mac-mgmt-server-skill-center mac-mgmt-server-skill-importer mac-mgmt-relay mac-mgmt-runner mmr-causality mac-mgmt-relay-ssh web-agency-server web-agency-proxy nix-driver-sync;
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
            skopeo

            # Dioxus CLI (patched with --skip-platform-features)
            dioxus-cli-patched
            rcodesign  # ad-hoc MachO signing when cross-compiling from Linux

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
            wasm-bindgen-cli_0_2_121
            binaryen  # wasm-opt
            lld

          ] ++ darwinDeps
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            # Native overview desktop app (mac-mgmt --features usb-ui →
            # dioxus desktop / wry). Linux webview stack + GObject deps.
            pkgs.webkitgtk_4_1
            pkgs.gtk3
            pkgs.libsoup_3
            pkgs.glib-networking
            pkgs.glib
            pkgs.cairo
            pkgs.pango
            pkgs.atk
            pkgs.gdk-pixbuf
            pkgs.librsvg
            pkgs.xdotool       # libxdo — required by the wry/dioxus-desktop link
            pkgs.wrapGAppsHook3
          ];

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
          mmr-causality = mmr-causality;
          relay-ssh = relay-ssh;
          web-agency = web-agency-server;
          web-agency-proxy = web-agency-proxy;
          macosx-sdk = macosx-sdk;
          nix-driver-sync = nix-driver-sync;
          dioxus-cli-patched = pkgs.dioxus-cli-patched;
          memvault-extract-guest-text-wasm = pkgs.memvault-extract-guest-text-wasm;
          memvault-extract-guest-pdfrender-wasm = pkgs.memvault-extract-guest-pdfrender-wasm;
          memvault-extract-guest-ocr-wasm = pkgs.memvault-extract-guest-ocr-wasm;
          memvault-extract-guest-audio-wasm = pkgs.memvault-extract-guest-audio-wasm;
          memvault-extract-guest-wasm = pkgs.memvault-extract-guest-wasm;
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
          # Sovereign-AI USB build: root image extraction + private /nix mount +
          # offline run, with no host nix (nix.enable = false).
          usb = pkgs.callPackage ./tests/usb.nix { };

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
