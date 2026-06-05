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
    nativeBuildInputs = [ pkgs.lld pkgs.pkg-config ];
    env.MEMVAULT_EXTRACT_GUEST_WASM = "${pkgs.memvault-extract-guest-wasm}/memvault_extract_guest.wasm";
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
    machine.succeed(f"mkdir -p {stick}")

    # ── Prefetch (online): nix-portable + ext4 image + .nar cache ──────────
    machine.succeed(f"mac-mgmt usb-prefetch --home {stick} 2>&1 | tee /tmp/prefetch.log")
    machine.succeed(f"test -f {stick}/nix-store.img")
    machine.succeed(f"test -f {stick}/.local/bin/nix-portable")
    machine.succeed(f"test -d {stick}/nix-cache")
    machine.log("prefetch produced nix-portable + ext4 image + nar cache")

    # ── Offline run: private namespace + /nix mount + control API ──────────
    # Cut the network to prove no fetch happens.
    machine.succeed("systemctl stop systemd-networkd || true")
    machine.execute(
        f"mac-mgmt usb --headless --offline --home {stick} >/tmp/usb.log 2>&1 &"
    )

    # The control API binds an ephemeral [::1] port; discover it from the log.
    def control_port():
        rc, out = machine.execute(
            "grep -oE 'control/status API on http://\\[::1\\]:[0-9]+' /tmp/usb.log | grep -oE '[0-9]+$' | head -1"
        )
        return out.strip()

    port = ""
    for _ in range(120):
        port = control_port()
        if port:
            break
        time.sleep(1)
    assert port, "usb stack did not report a control port\n" + machine.succeed("tail -50 /tmp/usb.log || true")
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
