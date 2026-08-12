# NixOS VM test for the sovereign-AI USB build.
#
# Validates the root-requiring, no-host-nix path that unit/hermetic tests can't:
#   - `nix.enable = false`  -> no nix daemon, no `nix` command on PATH, so the
#     daemon takes the Portable runtime (nix-portable), not the HostNix fast path.
#   - `usb prefetch` (online) downloads nix-portable and creates the on-stick
#     ext4 store image + .nar cache (root).
#   - `usb --headless --offline` enters a private mount namespace and loop-mounts
#     the image at /nix, then serves the loopback control API reporting offline.
#   - The host's /nix/store is untouched before/after (namespace isolation).
#
# Run with:  nix build .#checks.x86_64-linux.usb -L
{ pkgs, ... }:

let
  nixPortableX86_64 = pkgs.fetchurl {
    url = "https://github.com/DavHau/nix-portable/releases/download/v012/nix-portable-x86_64";
    sha256 = "0an72ija1m1kxifim8qvvnfsi1lz659i5yx3xlxaq2f90icwa2dl";
  };
  nixpkgsTarball = pkgs.fetchurl {
    url = "https://github.com/NixOS/nixpkgs/archive/d99b013d5d1931ad77fe3912ed218170dec5d9a4.tar.gz";
    sha256 = "1fqrzzak2x2igmh960f3qgj1y01scq4ff58dbkql04drycjg1alh";
  };

  # Build mac-mgmt with the headless USB feature (no usb-ui: the VM has no
  # display). Distribution builds use static musl; here a normal build is enough
  # to exercise the mechanics — its libs are mmap'd before /nix is overmounted.
  mac-mgmt-usb = pkgs.rustPlatform.buildRustPackage {
    pname = "mac-mgmt-usb";
    version = "0.1.0";
    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;
    cargoLock.outputHashes = import ../extra-hashes.nix;
    cargoBuildFlags = [ "-p" "mac-mgmt" "--no-default-features" "--features" "usb" ];
    postPatch = ''
      cp -rL design design-canonical
      rm design
      mv design-canonical design
      substituteInPlace memvault/crates/memvault-web/Cargo.toml \
        --replace-fail 'path = "../../plan-ai-design"' 'path = "../../../design"'
    '';
    # The mac-mgmt USB binary depends on memvault-web even in headless mode, and
    # memvault-web's build script regenerates Tailwind CSS with `npm run
    # tailwind:build`. Keep the Node/Tailwind tools in this derivation so CI
    # fails at the real USB integration boundary instead of during build-script
    # tool discovery.
    nativeBuildInputs = [ pkgs.lld pkgs.pkg-config pkgs.nodejs pkgs.tailwindcss_3 ];
    env.MEMVAULT_EXTRACT_GUEST_TEXT_WASM = "${pkgs.memvault-extract-guest-text-wasm}/memvault_extract_guest_text.wasm";
    env.MEMVAULT_EXTRACT_GUEST_PDFRENDER_WASM = "${pkgs.memvault-extract-guest-pdfrender-wasm}/memvault_extract_guest_pdfrender.wasm";
    env.MEMVAULT_EXTRACT_GUEST_OCR_WASM = "${pkgs.memvault-extract-guest-ocr-wasm}/memvault_extract_guest_ocr.wasm";
    env.MEMVAULT_EXTRACT_GUEST_AUDIO_WASM = "${pkgs.memvault-extract-guest-audio-wasm}/memvault_extract_guest_audio.wasm";
    doCheck = false;
  };
in

pkgs.testers.nixosTest {
  name = "usb";

  nodes.machine = { lib, ... }: {
    # No host nix: forces the Portable runtime (nix-portable + ext4 image).
    nix.enable = false;

    environment.systemPackages = [
      mac-mgmt-usb
      pkgs.curl
      pkgs.e2fsprogs   # mkfs.ext4
      pkgs.util-linux  # mount / unshare
    ];

    # nix-portable runs builds via proot; make sure it is available.
    virtualisation.memorySize = 2048;
    virtualisation.diskSize = 4096;
    networking.firewall.enable = false;
  };

  testScript = ''
    import time

    machine.wait_for_unit("multi-user.target")

    # No host nix command (nix.enable = false) -> Portable path.
    machine.fail("command -v nix")

    # The host store is populated; capture a marker to compare against later.
    host_store_before = machine.succeed("ls -1 /nix/store | wc -l").strip()
    machine.log(f"host /nix/store entries before: {host_store_before}")

    stick = "/root/stick"
    machine.succeed(f"mkdir -p {stick}/.local/bin {stick}/.cache/nixpkgs")

    # NixOS VM tests intentionally do not rely on guest internet access. Seed
    # the artifacts that `usb-prefetch` would otherwise download so the check
    # still exercises the prefetch, image setup, and offline runtime paths.
    machine.succeed(
        f"install -m 0755 ${nixPortableX86_64} {stick}/.local/bin/nix-portable-x86_64-linux"
    )
    machine.succeed(
        f"cp ${nixpkgsTarball} {stick}/.cache/nixpkgs/nixpkgs-d99b013d5d1931ad77fe3912ed218170dec5d9a4.tar.gz"
    )

    # ── Prefetch (seeded online artifacts): nix-portable + ext4 image + .nar cache ──────────
    machine.succeed(f"mac-mgmt usb-prefetch --home {stick} 2>&1 | tee /tmp/prefetch.log")
    machine.succeed(f"test -f {stick}/nix-store.img")
    machine.succeed(f"test -f {stick}/.local/bin/nix-portable")
    machine.succeed(f"test -d {stick}/nix-cache")
    machine.log("prefetch produced nix-portable + ext4 image + nar cache")

    # ── Offline run: private namespace + /nix mount + control API ──────────
    # Cut the network to prove no fetch happens.
    machine.succeed("systemctl stop systemd-networkd || true")
    machine.execute(
        f"mac-mgmt usb --headless --offline --home {stick} >/tmp/usb.log 2>&1 & echo $! >/tmp/usb.pid"
    )

    # The control API binds an ephemeral [::1] port; discover it from the log.
    def control_port():
        rc, out = machine.execute(
            "grep -oE 'control/status API on http://\\[::1\\]:[0-9]+' /tmp/usb.log | grep -oE '[0-9]+$' | head -1"
        )
        return out.strip()

    def usb_diagnostics():
        return machine.succeed(
            """
            echo '--- usb process ---'
            if test -r /tmp/usb.pid; then
              pid=$(cat /tmp/usb.pid)
              echo "pid=$pid"
              ps -o pid,ppid,stat,etime,comm,args -p "$pid" || true
            else
              echo 'no /tmp/usb.pid'
            fi
            echo '--- recent mac-mgmt processes ---'
            ps -eo pid,ppid,stat,etime,comm,args | grep '[m]ac-mgmt usb' || true
            echo '--- loop devices ---'
            losetup -a || true
            echo '--- /nix mount ---'
            mount | grep ' on /nix ' || true
            echo '--- usb.log ---'
            tail -200 /tmp/usb.log || true
            """
        )

    port = ""
    # Slow CI runners may execute the VM without KVM, making nix-portable
    # extraction and the private loop mount much slower than local runs. Keep
    # polling the real readiness signal, but fail early if the process exits and
    # print enough state to debug mount/runtime hangs.
    for attempt in range(420):
        port = control_port()
        if port:
            break
        rc, _ = machine.execute("test -r /tmp/usb.pid && kill -0 $(cat /tmp/usb.pid)")
        if rc != 0:
            raise AssertionError("usb stack exited before reporting a control port\n" + usb_diagnostics())
        if attempt > 0 and attempt % 30 == 0:
            machine.log(f"waiting for usb control API ({attempt}s elapsed)")
        time.sleep(1)
    assert port, "usb stack did not report a control port before timeout\n" + usb_diagnostics()
    machine.log(f"usb control API on port {port}")

    # /status reports offline.
    status = machine.succeed(f"curl -sf http://[::1]:{port}/status")
    machine.log(f"status: {status}")
    assert '"offline":true' in status.replace(" ", ""), f"expected offline status, got {status}"

    # The image is mounted at /nix inside the stack's private namespace, but the
    # host's /nix/store is unchanged (mount was private to the daemon).
    host_store_after = machine.succeed("ls -1 /nix/store | wc -l").strip()
    assert host_store_after == host_store_before, (
        f"host /nix/store changed: {host_store_before} -> {host_store_after}"
    )
    machine.log("host /nix/store untouched — namespace isolation verified")
  '';
}
