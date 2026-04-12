# NixOS integration test for the SSH relay.
#
# Tests the full relay flow using real mac-mgmt components:
#   1. PostgreSQL database with seeded cluster + token
#   2. mac-mgmt-server validates tokens via DB (managed by NixOS module)
#   3. mac-mgmt-relay bridges SSH ↔ WebSocket
#   4. mac-mgmt daemon connects to relay, provides SSH via russh
#   5. An SSH client connects through the relay port
#   6. The SSH session runs a command and returns output
#
# Run with:  nix build .#checks.x86_64-linux.relay-integration -L
{
  pkgs,
  mac-mgmt-relay,
  ...
}:

let
  syncToken = "test-sync-token-abc123";
  settingToken = "test-setting-token-abc123";
  clusterId = "550e8400-e29b-41d4-a716-446655440000";

  # Build the daemon without the services feature so it skips
  # ollama/openclaw/mcporter management — only relay + SSH are needed.
  mac-mgmt-daemon = pkgs.rustPlatform.buildRustPackage {
    pname = "mac-mgmt-daemon";
    version = "0.1.0";
    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;
    cargoBuildFlags = [ "-p" "mac-mgmt" "--no-default-features" "--features" "relay" ];
    doCheck = false;
  };

  # API-only server build — no webui/WASM, just Rocket + migrations.
  # Override the full dx build to a plain cargo build without webui.
  mac-mgmt-server-api = (pkgs.callPackage ../server/package.nix { }).overrideAttrs (old: {
    cargoBuildFlags = [ "-p" "mac-mgmt-server" "--no-default-features" "--features" "server-api-only" ];
    # Reset to default cargo buildPhase/installPhase (remove dx build overrides)
    buildPhase = null;
    installPhase = null;
  });

  # Pre-generate an SSH keypair for the test
  testKeyDir = pkgs.runCommand "test-ssh-keys" {} ''
    mkdir -p $out
    ${pkgs.openssh}/bin/ssh-keygen -t ed25519 -f $out/id_ed25519 -N "" -q
  '';

  relayConfig = pkgs.writeText "relay.toml" ''
    listen_addr = "127.0.0.1:8080"
    ssh_port_min = 30000
    ssh_port_max = 30010
    server_api_url = "http://127.0.0.1:7378"
  '';

  # Daemon config — no server.url so remote config fetch is skipped.
  # server.token is used for relay authentication.
  daemonConfig = pkgs.writeText "daemon-config.toml" ''
    [metrics]
    port = 9396

    [server]
    url = "http://127.0.0.1:7378"
    token = "${syncToken}"

    [relay]
    url = "ws://127.0.0.1:8080"
    remote_ssh_enabled = true
  '';

  # Seed script — inserts cluster + hashed token into PostgreSQL
  seedScript = pkgs.writeScript "seed-db.py" ''
    #!${pkgs.python3}/bin/python3
    import hashlib, subprocess, sys

    sync_hash = hashlib.sha256(b"${syncToken}").hexdigest()
    setting_hash = hashlib.sha256(b"${settingToken}").hexdigest()

    sql = f"""
    INSERT INTO clusters (id, name) VALUES ('${clusterId}', 'test-cluster');
    INSERT INTO tokens (cluster_id, token_hash, kind, label)
      VALUES ('${clusterId}', '{sync_hash}', 'sync', 'test-sync');
    INSERT INTO tokens (cluster_id, token_hash, kind, label)
      VALUES ('${clusterId}', '{setting_hash}', 'setting', 'test-setting');
    """

    result = subprocess.run(
        ["psql", "-d", "mac-mgmt", "-c", sql],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(f"seed failed: {result.stderr}", file=sys.stderr)
        sys.exit(1)
    print("database seeded")
  '';
in

pkgs.testers.nixosTest {
  name = "relay-integration";

  nodes.machine = { lib, ... }: {
    imports = [ ../server/module.nix ];

    environment.systemPackages = [
      mac-mgmt-daemon
      mac-mgmt-relay
      pkgs.openssh
      pkgs.curl
      pkgs.python3
    ];

    services.mac-mgmt-server = {
      enable = true;
      package = mac-mgmt-server-api;
      settings = {
        database.url = "postgres:///mac-mgmt?host=/run/postgresql";
        api.port = 7378;
        web.port = 7377;
      };
    };

    # Allow the DynamicUser service to connect to PostgreSQL
    services.postgresql.authentication = lib.mkForce ''
      local all all trust
      host all all 127.0.0.1/32 trust
    '';

    networking.firewall.enable = false;
  };

  testScript = ''
    import json
    import time

    machine.wait_for_unit("postgresql.service")
    machine.wait_for_unit("mac-mgmt.service")
    machine.wait_for_open_port(7378)
    machine.log("mac-mgmt-server API started on port 7378")

    # Seed cluster + token into the database
    machine.succeed("sudo -u postgres ${seedScript}")
    machine.log("Database seeded with test cluster and token")

    # Verify token works via /api/self
    self_json = machine.succeed(
        "curl -sf -H 'Authorization: Bearer ${settingToken}' http://127.0.0.1:7378/api/self"
    )
    self_info = json.loads(self_json)
    assert self_info["cluster_name"] == "test-cluster", f"unexpected self info: {self_info}"
    assert self_info["token_kind"] == "setting", f"unexpected token kind: {self_info}"
    machine.log("Token validation via mac-mgmt-server verified")

    # Start the relay
    machine.execute(
        "mac-mgmt-relay -c ${relayConfig} >/tmp/relay.log 2>&1 &"
    )
    machine.wait_for_open_port(8080)
    machine.log("Relay started on port 8080")

    # Verify health endpoint
    machine.succeed("curl -sf http://127.0.0.1:8080/health")
    machine.log("Relay health check passed")

    # Set up the daemon environment
    machine.succeed(
        "mkdir -p /root/.config/mac-mgmt && "
        "cp ${daemonConfig} /root/.config/mac-mgmt/config.toml && "
        "cp ${testKeyDir}/id_ed25519.pub /root/.config/mac-mgmt/authorized_keys"
    )

    # Start the daemon (built without services feature — no ollama/openclaw needed)
    machine.execute(
        "mac-mgmt daemon >/tmp/daemon.log 2>&1 &"
    )

    # Remote SSH is enabled in daemon config (remote_ssh_enabled = true) so
    # the FIFO toggle is not needed. We wait for the FIFO file as a "daemon
    # main loop is running" signal — wait_for_open_port(9396) is unreliable
    # because the daemon's metrics server binds to ::1 only.
    machine.wait_for_file("/root/.config/mac-mgmt/remote-ssh")
    machine.log("Daemon main loop reached (FIFO created)")

    # Wait for the daemon to register with the relay (port file isn't written,
    # so we poll the tunnel list API instead)
    attempts = 0
    relay_port = None
    while attempts < 60:
        try:
            tunnels_json = machine.succeed(
                "curl -sf -H 'Authorization: Bearer ${settingToken}' "
                "http://127.0.0.1:8080/api/tunnels"
            )
            tunnels = json.loads(tunnels_json)
            if len(tunnels) > 0:
                relay_port = tunnels[0]["ssh_port"]
                break
        except Exception:
            pass
        time.sleep(1)
        attempts += 1

    assert relay_port is not None, "daemon did not register with relay within 60s"
    machine.log(f"Daemon registered, relay SSH port: {relay_port}")

    # Verify tunnel metadata from real server
    tunnels = json.loads(tunnels_json)
    assert len(tunnels) == 1, f"expected 1 tunnel, got {len(tunnels)}: {tunnels}"
    assert tunnels[0]["cluster_name"] == "test-cluster"
    machine.log("Tunnel list API verified with real server auth")

    # SSH through the relay to the daemon's russh server
    machine.succeed(
        "cp ${testKeyDir}/id_ed25519 /tmp/test_key && chmod 600 /tmp/test_key"
    )
    machine.wait_for_open_port(relay_port)
    time.sleep(1)

    # Spawn two SSH sessions in parallel and wait for both to finish.
    # This exercises session multiplexing through the relay.
    parallel_cmd = (
        f"set -e; "
        f"ssh -p {relay_port} -i /tmp/test_key "
        f"-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        f"-o ConnectTimeout=10 "
        f"root@127.0.0.1 'echo SESSION_A_OK' >/tmp/ssh-a.out 2>/tmp/ssh-a.err & "
        f"PID_A=$!; "
        f"ssh -p {relay_port} -i /tmp/test_key "
        f"-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        f"-o ConnectTimeout=10 "
        f"root@127.0.0.1 'echo SESSION_B_OK' >/tmp/ssh-b.out 2>/tmp/ssh-b.err & "
        f"PID_B=$!; "
        f"wait $PID_A; RC_A=$?; "
        f"wait $PID_B; RC_B=$?; "
        f"echo \"RC_A=$RC_A RC_B=$RC_B\"; "
        f"exit $((RC_A + RC_B))"
    )
    rc, out = machine.execute(parallel_cmd, timeout=60)
    machine.log(f"Parallel SSH exit code: {rc}")
    machine.log(f"Parallel SSH summary: {out.strip()}")
    if rc != 0:
        machine.log("--- /tmp/ssh-a.err ---")
        machine.log(machine.succeed("cat /tmp/ssh-a.err || true"))
        machine.log("--- /tmp/ssh-b.err ---")
        machine.log(machine.succeed("cat /tmp/ssh-b.err || true"))
        machine.log("--- /tmp/relay.log ---")
        machine.log(machine.succeed("cat /tmp/relay.log || true"))
        machine.log("--- /tmp/daemon.log ---")
        machine.log(machine.succeed("cat /tmp/daemon.log || true"))
    assert rc == 0, f"parallel SSH sessions failed (rc={rc}): {out!r}"

    out_a = machine.succeed("cat /tmp/ssh-a.out")
    out_b = machine.succeed("cat /tmp/ssh-b.out")
    assert "SESSION_A_OK" in out_a, f"session A output: {out_a!r}"
    assert "SESSION_B_OK" in out_b, f"session B output: {out_b!r}"
    machine.log("Both parallel SSH sessions through relay succeeded")

    machine.log("All relay integration tests passed!")
  '';
}
