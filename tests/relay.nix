# NixOS integration test for the libp2p relay.
#
# Tests the full relay flow using real mac-mgmt components:
#   1. PostgreSQL database with seeded cluster + token
#   2. mac-mgmt-server validates tokens via DB (managed by NixOS module)
#   3. mac-mgmt-relay provides libp2p circuit relay + HTTP proxy
#   4. mac-mgmt daemon connects to relay via libp2p, registers, advertises tunnels
#   5. Verify daemon registration, tunnel advertisement, and proxy_url in heartbeat
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
    cargoLock.outputHashes = import ../extra-hashes.nix;
    cargoBuildFlags = [ "-p" "mac-mgmt" "--no-default-features" "--features" "relay" ];
    doCheck = false;
  };

  # API-only server build — no webui/WASM, just Rocket + migrations.
  mac-mgmt-server-api = (pkgs.callPackage ../server/package.nix { }).overrideAttrs (old: {
    cargoBuildFlags = [ "-p" "mac-mgmt-server" "--no-default-features" "--features" "server-api-only" ];
    buildPhase = null;
    installPhase = null;
  });

  # Pre-generate an SSH keypair for the test
  testKeyDir = pkgs.runCommand "test-ssh-keys" {} ''
    mkdir -p $out
    ${pkgs.openssh}/bin/ssh-keygen -t ed25519 -f $out/id_ed25519 -N "" -q
  '';

  # Release relay builds require TLS material. Generate a local test CA and a
  # relay server certificate signed by that CA so test clients can verify TLS
  # normally, without disabling certificate validation.
  testTlsDir = pkgs.runCommand "test-relay-tls" {} ''
    mkdir -p $out
    ${pkgs.openssl}/bin/openssl req -x509 -newkey rsa:2048 \
      -keyout $out/ca-key.pem \
      -out $out/ca-cert.pem \
      -days 1 \
      -nodes \
      -subj "/CN=mac-mgmt relay integration test CA" \
      -addext "basicConstraints=critical,CA:TRUE" \
      -addext "keyUsage=critical,keyCertSign,cRLSign"
    ${pkgs.openssl}/bin/openssl req -newkey rsa:2048 \
      -keyout $out/key.pem \
      -out $out/cert.csr \
      -nodes \
      -subj "/CN=127.0.0.1" \
      -addext "subjectAltName=IP:127.0.0.1,DNS:localhost"
    ${pkgs.openssl}/bin/openssl x509 -req \
      -in $out/cert.csr \
      -CA $out/ca-cert.pem \
      -CAkey $out/ca-key.pem \
      -CAcreateserial \
      -out $out/cert.pem \
      -days 1 \
      -copy_extensions copyall
    rm $out/cert.csr
  '';

  testCaModule = { ... }: {
    security.pki.certificateFiles = [ "${testTlsDir}/ca-cert.pem" ];
  };

  relayConfig = pkgs.writeText "relay.toml" ''
    listen_addr = "127.0.0.1:8080"
    server_api_url = "http://127.0.0.1:7378"
    proxy_url = "https://127.0.0.1:8080"
    data_dir = "/tmp/mac-mgmt-relay-data"
    p2p_port = 4001
    tls_cert_path = "${testTlsDir}/cert.pem"
    tls_key_path = "${testTlsDir}/key.pem"
  '';

  # Daemon config — connects to relay via libp2p WS on the HTTP port.
  daemonConfig = pkgs.writeText "daemon-config.toml" ''
    [metrics]
    port = 9396

    [server]
    url = "http://127.0.0.1:7378"
    token = "${syncToken}"

    [relay]
    relay_multiaddr = "/ip4/127.0.0.1/tcp/4001/ws"
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
    imports = [
      ../server/module.nix
      testCaModule
    ];

    nix.settings.experimental-features = [ "nix-command" "flakes" ];

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

    # Start the relay and fail with its log if it exits before becoming ready.
    machine.execute(
        "RUST_LOG=info mac-mgmt-relay -c ${relayConfig} >/tmp/relay.log 2>&1 & echo $! >/tmp/relay.pid"
    )
    attempts = 0
    while attempts < 120:
        if machine.execute("curl -sf https://127.0.0.1:8080/health >/dev/null")[0] == 0:
            break
        if machine.execute("kill -0 $(cat /tmp/relay.pid) 2>/dev/null")[0] != 0:
            relay_log = machine.succeed("cat /tmp/relay.log || true")
            raise Exception(f"relay exited before readiness:\n{relay_log}")
        time.sleep(1)
        attempts += 1
    else:
        relay_log = machine.succeed("cat /tmp/relay.log || true")
        raise Exception(f"relay did not become ready on port 8080:\n{relay_log}")
    machine.log("Relay started on port 8080 and health check passed")

    # Set up the daemon environment
    machine.succeed(
        "mkdir -p /root/.config/mac-mgmt && "
        "cp ${daemonConfig} /root/.config/mac-mgmt/config.toml && "
        "cp ${testKeyDir}/id_ed25519.pub /root/.config/mac-mgmt/authorized_keys"
    )

    # Start the daemon
    machine.execute(
        "mac-mgmt daemon >/tmp/daemon.log 2>&1 &"
    )

    # Wait for the FIFO file as a "daemon main loop is running" signal.
    machine.wait_for_file("/root/.config/mac-mgmt/remote-ssh")
    machine.log("Daemon main loop reached (FIFO created)")

    # Wait for the daemon to register with the relay via libp2p.
    # Poll the tunnel list API — the daemon should appear once its
    # RPC stream registration completes.
    attempts = 0
    instance_id = None
    while attempts < 120:
        try:
            tunnels_json = machine.succeed(
                "curl -sf -H 'Authorization: Bearer ${settingToken}' "
                "https://127.0.0.1:8080/api/tunnels"
            )
            tunnels = json.loads(tunnels_json)
            if len(tunnels) > 0:
                instance_id = tunnels[0]["instance_id"]
                break
        except Exception:
            pass
        time.sleep(1)
        attempts += 1

    if instance_id is None:
        machine.log("daemon did not register; dumping logs:")
        machine.log(machine.succeed("cat /tmp/relay.log || true"))
        machine.log(machine.succeed("cat /tmp/daemon.log || true"))
    assert instance_id is not None, "daemon did not register with relay within 120s"
    machine.log(f"Daemon registered with instance_id: {instance_id}")

    # Verify tunnel metadata
    tunnels = json.loads(tunnels_json)
    assert len(tunnels) == 1, f"expected 1 tunnel entry, got {len(tunnels)}: {tunnels}"
    machine.log("Tunnel list API verified")

    # Verify the daemon sent relay_proxy_url in its heartbeat.
    # Poll the daemon_heartbeats row directly — there is no public read API
    # for heartbeats and the daemon writes asynchronously after registration.
    relay_proxy_url = ""
    for _ in range(60):
        relay_proxy_url = machine.succeed(
            "sudo -u postgres psql -d mac-mgmt -t -A -c "
            "\"SELECT relay_proxy_url FROM daemon_heartbeats "
            f"WHERE cluster_id = '${clusterId}' AND instance_id = '{instance_id}'\""
        ).strip()
        if relay_proxy_url:
            break
        time.sleep(1)
    assert relay_proxy_url, \
        f"daemon never wrote relay_proxy_url for instance_id={instance_id}"
    machine.log(f"Heartbeat relay_proxy_url={relay_proxy_url}")

    machine.log("All relay integration tests passed!")
  '';
}
