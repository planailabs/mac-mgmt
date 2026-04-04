# NixOS integration test for the SSH relay.
#
# Tests the full relay flow using real mac-mgmt components:
#   1. PostgreSQL database with seeded customer + token
#   2. mac-mgmt-server validates tokens via DB
#   3. mac-mgmt-relay bridges SSH ↔ WebSocket
#   4. mac-mgmt daemon connects to relay, provides SSH via russh
#   5. An SSH client connects through the relay port
#   6. The SSH session runs a command and returns output
#
# Run with:  nix build .#checks.x86_64-linux.relay-integration -L
{
  pkgs,
  mac-mgmt-server,
  mac-mgmt-relay,
  ...
}:

let
  testToken = "test-token-abc123";
  customerId = "550e8400-e29b-41d4-a716-446655440000";

  # Build the daemon without the services feature so it skips
  # ollama/openclaw/mcporter management — only relay + SSH are needed.
  mac-mgmt-daemon = pkgs.rustPlatform.buildRustPackage {
    pname = "mac-mgmt-daemon";
    version = "0.1.0";
    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;
    cargoBuildFlags = [ "-p" "mac-mgmt" "--no-default-features" ];
    doCheck = false;
  };

  # Pre-generate an SSH keypair for the test
  testKeyDir = pkgs.runCommand "test-ssh-keys" {} ''
    mkdir -p $out
    ${pkgs.openssh}/bin/ssh-keygen -t ed25519 -f $out/id_ed25519 -N "" -q
  '';

  # Server config (API only — web UI unused, OIDC skipped via DEV_ONLY_NO_AUTH)
  serverConfig = pkgs.writeText "server-config.toml" ''
    [database]
    url = "postgres:///mac_mgmt_test?host=/run/postgresql"

    [api]
    port = 7378

    [web]
    port = 7377

    [oidc]
    client_id = "unused"
    client_secret = "unused"
    redirect_uri = "http://localhost:7377/callback"
    cookie_secret = "0123456789abcdef0123456789abcdef"

    [xzar]
    url = "http://localhost:9999"
    token = "unused"
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

    [openclaw]
    provider = "ollama"

    [ollama]
    flavour = "cpu"
    models = []
    default_model = ""

    [server]
    token = "${testToken}"

    [relay]
    url = "ws://127.0.0.1:8080"
  '';

  # Seed script — inserts customer + hashed token into PostgreSQL
  seedScript = pkgs.writeScript "seed-db.py" ''
    #!${pkgs.python3}/bin/python3
    import hashlib, subprocess, sys

    token_hash = hashlib.sha256(b"${testToken}").hexdigest()

    sql = f"""
    INSERT INTO customers (id, name) VALUES ('${customerId}', 'test-customer');
    INSERT INTO tokens (customer_id, token_hash, kind, label)
      VALUES ('${customerId}', '{token_hash}', 'setting', 'test');
    """

    result = subprocess.run(
        ["psql", "-d", "mac_mgmt_test", "-c", sql],
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
    environment.systemPackages = [
      mac-mgmt-daemon
      mac-mgmt-server
      mac-mgmt-relay
      pkgs.openssh
      pkgs.curl
      pkgs.postgresql
      pkgs.python3
    ];

    services.postgresql = {
      enable = true;
      # Allow any local user to connect without password
      authentication = lib.mkForce ''
        local all all trust
        host all all 127.0.0.1/32 trust
      '';
    };

    networking.firewall.enable = false;
  };

  testScript = ''
    import json
    import time

    machine.wait_for_unit("postgresql.service")
    machine.succeed("sudo -u postgres createuser -s root || true")
    machine.succeed("createdb mac_mgmt_test || true")

    # 1. Start mac-mgmt-server (runs DB migrations on startup)
    machine.execute(
        "DEV_ONLY_NO_AUTH=1 CONFIG_PATH=${serverConfig} "
        "mac-mgmt-server >/tmp/server.log 2>&1 &"
    )
    machine.wait_for_open_port(7378)
    machine.log("mac-mgmt-server API started on port 7378")

    # 2. Seed customer + token into the database
    machine.succeed("${seedScript}")
    machine.log("Database seeded with test customer and token")

    # Verify token works via /api/self
    self_json = machine.succeed(
        "curl -sf -H 'Authorization: Bearer ${testToken}' http://127.0.0.1:7378/api/self"
    )
    self_info = json.loads(self_json)
    assert self_info["customer_name"] == "test-customer", f"unexpected self info: {self_info}"
    assert self_info["token_kind"] == "setting", f"unexpected token kind: {self_info}"
    machine.log("Token validation via mac-mgmt-server verified")

    # 3. Start the relay
    machine.execute(
        "mac-mgmt-relay -c ${relayConfig} >/tmp/relay.log 2>&1 &"
    )
    machine.wait_for_open_port(8080)
    machine.log("Relay started on port 8080")

    # Verify health endpoint
    machine.succeed("curl -sf http://127.0.0.1:8080/health")
    machine.log("Relay health check passed")

    # 4. Set up the daemon environment
    machine.succeed(
        "mkdir -p /root/.config/mac-mgmt/ssh && "
        "cp ${daemonConfig} /root/.config/mac-mgmt/config.toml && "
        "cp ${testKeyDir}/id_ed25519.pub /root/.config/mac-mgmt/ssh/authorized_keys"
    )

    # Start the daemon (built without services feature — no ollama/openclaw needed)
    machine.execute(
        "mac-mgmt daemon >/tmp/daemon.log 2>&1 &"
    )

    # Wait for the daemon's metrics server (indicates main loop is running)
    machine.wait_for_open_port(9396)
    machine.log("Daemon started, metrics server on port 9396")

    # 5. Enable remote SSH via FIFO
    # Wait for the FIFO to be created by the daemon
    machine.wait_for_file("/root/.config/mac-mgmt/remote-ssh")
    machine.succeed("echo enable > /root/.config/mac-mgmt/remote-ssh")
    machine.log("Sent 'enable' to remote SSH FIFO")

    # Wait for the daemon to register with the relay (port file isn't written,
    # so we poll the tunnel list API instead)
    retry = 0
    relay_port = None
    while retry < 60:
        try:
            tunnels_json = machine.succeed(
                "curl -sf -H 'Authorization: Bearer ${testToken}' "
                "http://127.0.0.1:8080/api/tunnels"
            )
            tunnels = json.loads(tunnels_json)
            if len(tunnels) > 0:
                relay_port = tunnels[0]["ssh_port"]
                break
        except Exception:
            pass
        time.sleep(1)
        retry += 1

    assert relay_port is not None, "daemon did not register with relay within 60s"
    machine.log(f"Daemon registered, relay SSH port: {relay_port}")

    # 6. Verify tunnel metadata from real server
    tunnels = json.loads(tunnels_json)
    assert len(tunnels) == 1, f"expected 1 tunnel, got {len(tunnels)}: {tunnels}"
    assert tunnels[0]["customer_name"] == "test-customer"
    machine.log("Tunnel list API verified with real server auth")

    # 7. SSH through the relay to the daemon's russh server
    machine.succeed(
        "cp ${testKeyDir}/id_ed25519 /tmp/test_key && chmod 600 /tmp/test_key"
    )
    machine.wait_for_open_port(relay_port)
    time.sleep(1)

    result = machine.succeed(
        f"ssh -p {relay_port} -i /tmp/test_key "
        f"-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        f"root@127.0.0.1 'echo RELAY_TEST_OK'"
    )
    assert "RELAY_TEST_OK" in result, f"SSH command output: {result}"
    machine.log("SSH through relay to daemon succeeded!")

    # 8. Verify a second session works (tests session multiplexing)
    result2 = machine.succeed(
        f"ssh -p {relay_port} -i /tmp/test_key "
        f"-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        f"root@127.0.0.1 'hostname'"
    )
    machine.log(f"Second SSH session returned: {result2.strip()}")

    machine.log("All relay integration tests passed!")
  '';
}
