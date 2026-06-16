# NixOS integration test for the relay's federated /metrics endpoint.
#
# Boots a single VM running:
#   1. PostgreSQL + mac-mgmt-server (token validation)
#   2. mac-mgmt-relay (the federated /metrics endpoint under test)
#   3. mac-mgmt daemon, built without the `services` feature so it does
#      not try to install/manage ollama/openclaw/etc — it only needs to
#      register with the relay and expose its own /metrics.
#   4. NixOS prometheus.service scraping the relay with a bearer token.
#
# The test asserts that prometheus successfully scrapes the relay and
# stores the synthetic federation metrics (`mac_mgmt_relay_scrape_up`,
# `mac_mgmt_relay_scrape_targets`) carrying `instance_id`, `hostname`
# and `cluster_id` labels for the connected daemon.
#
# Run with:  nix build .#checks.x86_64-linux.metrics-federation -L
{
  pkgs,
  mac-mgmt-relay,
  ...
}:

let
  syncToken = "test-sync-token-abc123";
  settingToken = "test-setting-token-abc123";
  clusterId = "550e8400-e29b-41d4-a716-446655440000";

  mac-mgmt-daemon = pkgs.rustPlatform.buildRustPackage {
    pname = "mac-mgmt-daemon";
    version = "0.1.0";
    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;
    cargoLock.outputHashes = import ../extra-hashes.nix;
    cargoBuildFlags = [ "-p" "mac-mgmt" "--no-default-features" "--features" "relay" ];
    nativeBuildInputs = [ pkgs.lld ];
    env.MEMVAULT_EXTRACT_GUEST_WASM = "${pkgs.memvault-extract-guest-wasm}/memvault_extract_guest.wasm";
    doCheck = false;
  };

  mac-mgmt-server-api = (pkgs.callPackage ../server/package.nix { }).overrideAttrs (old: {
    cargoBuildFlags = [ "-p" "mac-mgmt-server" "--no-default-features" "--features" "server-api-only" ];
    buildPhase = null;
    installPhase = null;
  });

  relayConfig = pkgs.writeText "relay.toml" ''
    listen_addr = "127.0.0.1:8080"
    server_api_url = "http://127.0.0.1:7378"
    proxy_url = "https://127.0.0.1:8080"
    data_dir = "/tmp/mac-mgmt-relay-data"
    p2p_port = 4001
    tls_cert_path = "${testTlsDir}/cert.pem"
    tls_key_path = "${testTlsDir}/key.pem"
  '';

  testKeyDir = pkgs.runCommand "test-ssh-keys" {} ''
    mkdir -p $out
    ${pkgs.openssh}/bin/ssh-keygen -t ed25519 -f $out/id_ed25519 -N "" -q
  '';

  # Release relay builds require TLS material. Generate a local test CA and a
  # relay server certificate signed by that CA so curl and Prometheus can
  # verify HTTPS normally instead of relying on certificate bypasses. Keep the
  # validity window long because this derivation can be reused from the Nix
  # store/cache well after it was originally built.
  testTlsDir = pkgs.runCommand "test-relay-tls" {} ''
    mkdir -p $out
    ${pkgs.openssl}/bin/openssl req -x509 -newkey rsa:2048 \
      -keyout $out/ca-key.pem \
      -out $out/ca-cert.pem \
      -days 3650 \
      -nodes \
      -subj "/CN=mac-mgmt metrics federation test CA" \
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
      -days 3650 \
      -copy_extensions copyall
    rm $out/cert.csr
  '';

  testCaModule = { ... }: {
    security.pki.certificateFiles = [ "${testTlsDir}/ca-cert.pem" ];
  };

  daemonConfig = pkgs.writeText "daemon-config.toml" ''
    [metrics]
    port = 9396

    [server]
    url = "http://127.0.0.1:7378"
    token = "${syncToken}"

    [relay]
    relay_multiaddr = "/ip4/127.0.0.1/tcp/4001/ws"
  '';

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
  name = "metrics-federation";

  nodes.machine = { lib, ... }: {
    imports = [
      ../server/module.nix
      testCaModule
    ];

    nix.settings.experimental-features = [ "nix-command" "flakes" ];

    environment.systemPackages = [
      mac-mgmt-daemon
      mac-mgmt-relay
      pkgs.curl
      pkgs.jq
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

    services.prometheus = {
      enable = true;
      port = 9090;
      globalConfig = {
        scrape_interval = "3s";
        evaluation_interval = "3s";
      };
      scrapeConfigs = [{
        job_name = "mac-mgmt-relay";
        metrics_path = "/metrics";
        scheme = "https";
        bearer_token = settingToken;
        tls_config = {
          ca_file = "${testTlsDir}/ca-cert.pem";
        };
        scrape_interval = "3s";
        scrape_timeout = "2s";
        static_configs = [{
          targets = [ "127.0.0.1:8080" ];
        }];
      }];
    };

    networking.firewall.enable = false;
  };

  testScript = ''
    import json
    import time
    import urllib.parse

    def prom_query(q):
        encoded = urllib.parse.quote(q)
        out = machine.succeed(
            f"curl -sf 'http://127.0.0.1:9090/api/v1/query?query={encoded}'"
        )
        return json.loads(out)

    machine.wait_for_unit("postgresql.service")
    machine.wait_for_unit("mac-mgmt.service")
    machine.wait_for_open_port(7378)
    machine.log("mac-mgmt-server API started on port 7378")

    machine.succeed("sudo -u postgres ${seedScript}")
    machine.log("Database seeded")

    # Verify token works via /api/self
    self_json = machine.succeed(
        "curl -sf -H 'Authorization: Bearer ${settingToken}' http://127.0.0.1:7378/api/self"
    )
    assert json.loads(self_json)["cluster_name"] == "test-cluster"
    machine.log("Token validation verified via mac-mgmt-server")

    def dump_service_logs(context):
        machine.log(f"{context}; dumping relay/daemon/server logs:")
        machine.log("--- /tmp/relay.log ---")
        machine.log(machine.succeed("cat /tmp/relay.log || true"))
        machine.log("--- /tmp/daemon.log ---")
        machine.log(machine.succeed("cat /tmp/daemon.log || true"))
        machine.log("--- mac-mgmt.service journal ---")
        machine.log(machine.succeed("journalctl -u mac-mgmt.service --no-pager -n 200 || true"))

    relay_curl = "curl --cacert ${testTlsDir}/ca-cert.pem"

    def wait_for_pid_health(name, pid_file, health_cmd, log_file, attempts=120):
        last_health_error = ""
        for _ in range(attempts):
            status, out = machine.execute(health_cmd)
            if status == 0:
                return
            last_health_error = out
            if machine.execute(f"kill -0 $(cat {pid_file}) 2>/dev/null")[0] != 0:
                service_log = machine.succeed(f"cat {log_file} || true")
                raise Exception(
                    f"{name} exited before readiness; last health probe output:\n"
                    f"{last_health_error}\n{service_log}"
                )
            time.sleep(1)
        service_log = machine.succeed(f"cat {log_file} || true")
        verbose_health = machine.succeed(
            health_cmd.replace(" -sf ", " -sv ").replace(" >/dev/null", "") + " 2>&1 || true"
        )
        raise Exception(
            f"{name} did not become ready within {attempts}s; "
            f"last health probe output:\n{last_health_error}\n"
            f"verbose health probe:\n{verbose_health}\n{service_log}"
        )

    # Start the relay and poll its real health endpoint.  Do not rely only on
    # wait_for_open_port: if the relay exits early, dump its log immediately
    # instead of letting the VM test hang until the NixOS backdoor times out.
    machine.execute(
        "mac-mgmt-relay -c ${relayConfig} >/tmp/relay.log 2>&1 & echo $! >/tmp/relay.pid"
    )
    wait_for_pid_health(
        "mac-mgmt-relay",
        "/tmp/relay.pid",
        relay_curl + " -sf https://127.0.0.1:8080/health >/dev/null",
        "/tmp/relay.log",
    )
    machine.log("Relay running on port 8080")

    # Sanity-check the federated /metrics endpoint reachable + auth-gated
    machine.fail(relay_curl + " -sf https://127.0.0.1:8080/metrics")  # 401 — no token
    machine.succeed(
        relay_curl + " -sf -H 'Authorization: Bearer ${settingToken}' https://127.0.0.1:8080/metrics >/tmp/relay-metrics-empty.txt"
    )
    machine.log("Relay /metrics is reachable with bearer auth (no daemons yet)")

    # Wait for prometheus to come up
    machine.wait_for_unit("prometheus.service")
    machine.wait_for_open_port(9090)
    machine.log("Prometheus running on port 9090")

    # Start the daemon
    machine.succeed(
        "mkdir -p /root/.config/mac-mgmt/ssh && "
        "cp ${daemonConfig} /root/.config/mac-mgmt/config.toml && "
        "cp ${testKeyDir}/id_ed25519.pub /root/.config/mac-mgmt/ssh/authorized_keys"
    )
    machine.execute("mac-mgmt daemon >/tmp/daemon.log 2>&1 & echo $! >/tmp/daemon.pid")
    machine.log("Daemon kicked off")

    # Wait for the daemon to register with the relay via libp2p RPC stream.
    # Detect early daemon exits while polling so failures include daemon/relay logs.
    instance_id = None
    last_tunnels_error = None
    for _ in range(120):
        try:
            tunnels_json = machine.succeed(
                relay_curl + " -sf -H 'Authorization: Bearer ${settingToken}' "
                "https://127.0.0.1:8080/api/tunnels"
            )
            tunnels = json.loads(tunnels_json)
            if len(tunnels) > 0:
                instance_id = tunnels[0]["instance_id"]
                break
        except Exception as exc:
            last_tunnels_error = str(exc)
        if machine.execute("kill -0 $(cat /tmp/daemon.pid) 2>/dev/null")[0] != 0:
            dump_service_logs("daemon exited before registering with relay")
            raise Exception("daemon exited before registering with relay")
        time.sleep(1)
    if instance_id is None:
        dump_service_logs("daemon did not register with relay within 120s")
        raise Exception(
            "daemon did not register with relay within 120s; "
            f"last tunnels error: {last_tunnels_error}"
        )
    machine.log(f"Daemon registered: instance_id={instance_id}")

    # Hit the federated /metrics directly to confirm the daemon scrape now
    # produces synthetic relay metrics carrying the federation labels.
    direct = machine.succeed(
        relay_curl + " -sf -H 'Authorization: Bearer ${settingToken}' https://127.0.0.1:8080/metrics"
    )
    machine.log("Federated /metrics body (first 1KB):")
    machine.log(direct[:1024])
    assert "mac_mgmt_relay_scrape_targets 1" in direct, \
        f"expected scrape_targets=1 in body:\n{direct}"
    assert "mac_mgmt_relay_scrape_up{" in direct, \
        f"expected scrape_up series in body:\n{direct}"
    assert f'instance_id="{instance_id}"' in direct, \
        f"expected instance_id label in body:\n{direct}"
    assert 'cluster_id="${clusterId}"' in direct, \
        f"expected cluster_id label in body:\n{direct}"
    machine.log("Direct /metrics body carries federation labels")

    # Wait for Prometheus to scrape successfully at least once.
    target_up = False
    for _ in range(120):
        targets_json = machine.succeed(
            "curl -sf 'http://127.0.0.1:9090/api/v1/targets?state=active'"
        )
        targets = json.loads(targets_json)
        for target in targets["data"]["activeTargets"]:
            if target["labels"].get("job") == "mac-mgmt-relay" and target["health"] == "up":
                target_up = True
                break
        if target_up:
            break
        time.sleep(1)
    assert target_up, f"prometheus never scraped the relay successfully:\n{targets_json}"
    machine.log("Prometheus reports the relay scrape target as up")

    # Query Prometheus for the synthetic metrics.
    targets_value = 0.0
    for _ in range(60):
        result = prom_query("mac_mgmt_relay_scrape_targets")
        if result["status"] == "success" and result["data"]["result"]:
            targets_value = float(result["data"]["result"][0]["value"][1])
            if targets_value >= 1.0:
                break
        time.sleep(1)
    assert targets_value >= 1.0, \
        f"mac_mgmt_relay_scrape_targets never reached 1: {targets_value}"
    machine.log(f"Prometheus has mac_mgmt_relay_scrape_targets = {targets_value}")

    # Query mac_mgmt_relay_scrape_up filtered by the daemon we registered.
    up_q = (
        f'mac_mgmt_relay_scrape_up{{instance_id="{instance_id}",'
        'cluster_id="${clusterId}"}'
    )
    up_value = 0.0
    series_labels: dict = {}
    for _ in range(60):
        result = prom_query(up_q)
        if result["status"] == "success" and result["data"]["result"]:
            sample = result["data"]["result"][0]
            up_value = float(sample["value"][1])
            series_labels = sample["metric"]
            if up_value == 1.0:
                break
        time.sleep(1)

    assert up_value == 1.0, \
        f"mac_mgmt_relay_scrape_up did not reach 1 for our daemon: value={up_value}"
    assert series_labels.get("instance_id") == instance_id, \
        f"unexpected instance_id label: {series_labels}"
    assert series_labels.get("cluster_id") == "${clusterId}", \
        f"unexpected cluster_id label: {series_labels}"
    assert "hostname" in series_labels, \
        f"hostname label missing: {series_labels}"
    machine.log(
        "mac_mgmt_relay_scrape_up=1 with federation labels: " + json.dumps(series_labels)
    )

    # Check the duration metric is being recorded.
    dur = prom_query(
        f'mac_mgmt_relay_scrape_duration_seconds{{instance_id="{instance_id}"}}'
    )
    assert dur["status"] == "success" and dur["data"]["result"], \
        f"scrape_duration_seconds missing: {dur}"
    dur_value = float(dur["data"]["result"][0]["value"][1])
    assert dur_value >= 0.0, f"unexpected duration value: {dur_value}"
    machine.log(f"mac_mgmt_relay_scrape_duration_seconds = {dur_value}")

    machine.log("Federated metrics integration test passed!")
  '';
}
