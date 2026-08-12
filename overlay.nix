{ gitSha ? "unknown" }:

final: prev:
let
  # Patched dioxus-cli with:
  # - --skip-platform-features: prevents dx from auto-adding "web"/"desktop"/etc.
  # - --embed: embeds public assets into the server binary via rust-embed
  # - optional codesign: falls back to rcodesign for cross-compilation
  dioxus-cli-patched = prev.dioxus-cli.overrideAttrs (old: {
    patches = (old.patches or []) ++ [
      ./patches/dioxus-cli-all.patch
    ];
  });
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
  wasmToolchain = prev.rust-bin.stable.latest.default.override {
    targets = [ "wasm32-unknown-unknown" "wasm32-wasip1" ];
  };
  wasmRustPlatform = prev.makeRustPlatform {
    cargo = wasmToolchain;
    rustc = wasmToolchain;
  };
  mkMemvaultExtractGuestWasm = { crate, target }:
    wasmRustPlatform.buildRustPackage {
      pname = "${crate}-wasm";
      version = "0.1.0";
      src = ./.;
      cargoRoot = "memvault";
      buildAndTestSubdir = "memvault";
      cargoLock = {
        lockFile = ./memvault/Cargo.lock;
        outputHashes = import ./memvault/extra-hashes.nix;
      };
      nativeBuildInputs = [ prev.lld ];
      cargoBuildFlags = [
        "-p"
        crate
        "--target"
        target
      ];
      doCheck = false;
      installPhase = ''
        runHook preInstall
        mkdir -p $out
        cp target/${target}/release/${prev.lib.replaceStrings [ "-" ] [ "_" ] crate}.wasm \
          $out/${prev.lib.replaceStrings [ "-" ] [ "_" ] crate}.wasm
        runHook postInstall
      '';
    };
  memvaultExtractGuestTextWasm = mkMemvaultExtractGuestWasm {
    crate = "memvault-extract-guest-text";
    target = "wasm32-unknown-unknown";
  };
  memvaultExtractGuestPdfRenderWasm = mkMemvaultExtractGuestWasm {
    crate = "memvault-extract-guest-pdfrender";
    target = "wasm32-wasip1";
  };
  memvaultExtractGuestOcrWasm = mkMemvaultExtractGuestWasm {
    crate = "memvault-extract-guest-ocr";
    target = "wasm32-wasip1";
  };
  memvaultExtractGuestAudioWasm = mkMemvaultExtractGuestWasm {
    crate = "memvault-extract-guest-audio";
    target = "wasm32-wasip1";
  };
in
{
  python313 = prev.python313.override {
    packageOverrides = pyFinal: pyPrev: {
      torchao = pyPrev.torchao.overridePythonAttrs (old: {
        doCheck = false;
        env = (old.env or {}) // {
          VERSION_SUFFIX = "";
        };
      });
    };
  };

  inherit dioxus-cli-patched;
  memvault-extract-guest-text-wasm = memvaultExtractGuestTextWasm;
  memvault-extract-guest-pdfrender-wasm = memvaultExtractGuestPdfRenderWasm;
  memvault-extract-guest-ocr-wasm = memvaultExtractGuestOcrWasm;
  memvault-extract-guest-audio-wasm = memvaultExtractGuestAudioWasm;
  # Backwards-compatible alias for callers that only need the text extractor.
  memvault-extract-guest-wasm = memvaultExtractGuestTextWasm;

  mac-mgmt = prev.rustPlatform.buildRustPackage {
    pname = "mac-mgmt";
    version = "0.1.0";
    src = ./.;
    cargoLock.lockFile = ./Cargo.lock;
    cargoLock.outputHashes = import ./extra-hashes.nix;
    cargoTestFlags = [ "-p" "mac-mgmt" ];
    postPatch = ''
      cp -rL design design-canonical
      rm design
      mv design-canonical design
      substituteInPlace memvault/crates/memvault-web/Cargo.toml \
        --replace-fail 'path = "../../plan-ai-design"' 'path = "../../../design"'
    '';
    nativeBuildInputs = [
      prev.nodejs
      prev.tailwindcss_3
      dioxus-cli-patched
      prev.wasm-bindgen-cli_0_2_121
      prev.binaryen
      prev.lld
      prev.rcodesign
    ];
    buildInputs = prev.lib.optionals prev.stdenv.isDarwin [ prev.libiconv ];
    env.GIT_SHA = gitSha;
    env.MEMVAULT_EXTRACT_GUEST_TEXT_WASM = "${memvaultExtractGuestTextWasm}/memvault_extract_guest_text.wasm";
    env.MEMVAULT_EXTRACT_GUEST_PDFRENDER_WASM = "${memvaultExtractGuestPdfRenderWasm}/memvault_extract_guest_pdfrender.wasm";
    env.MEMVAULT_EXTRACT_GUEST_OCR_WASM = "${memvaultExtractGuestOcrWasm}/memvault_extract_guest_ocr.wasm";
    env.MEMVAULT_EXTRACT_GUEST_AUDIO_WASM = "${memvaultExtractGuestAudioWasm}/memvault_extract_guest_audio.wasm";
    # Fullstack build via dx: @client gets only the web feature (no native
    # deps), @server gets default features + `embed`. --embed bakes the
    # client's public assets into the server binary via rust-embed;
    # `@server --features embed` turns on the runtime gate that serves them.
    buildPhase = ''
      runHook preBuild

      # Preflight Cargo metadata before invoking dx.  dx has a short
      # cargo-metadata watchdog; on busy CI runners it can time out without
      # exposing the underlying Cargo failure.  Running metadata first warms
      # Cargo's cache and makes lock/network errors self-diagnosing.
      timeout 180 cargo metadata --format-version=1 --locked --no-deps >/dev/null

      # Tailwind CSS for memvault-web
      (cd memvault/crates/memvault-web && npm run tailwind:build)

      dx build --package mac-mgmt --release --embed \
        @client --platform web --no-default-features --features web \
        @server --platform server --features embed

      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      mkdir -p $out/bin
      cp target/dx/mac-mgmt/release/web/server $out/bin/mac-mgmt
      runHook postInstall
    '';
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

  mac-mgmt-relay = prev.callPackage ./relay/package.nix { };
  mac-mgmt-runner = prev.callPackage ./runner/package.nix { inherit gitSha; };
  mmr-causality = prev.callPackage ./mmr-causality/package.nix { inherit gitSha; };
  mac-mgmt-relay-ssh = prev.callPackage ./relay-ssh/package.nix { };
  nix-driver-sync = prev.callPackage ./nix-driver-sync/package.nix { };
}
