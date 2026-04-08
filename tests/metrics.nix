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
# and `customer_id` labels for the connected daemon.
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
  customerId = "550e8400-e29b-41d4-a716-446655440000";

  # Daemon without the services feature — it skips ollama/openclaw/mcporter
  # but keeps the `relay` feature so the relay client (always-on, metrics
  # requests pass through regardless of the SSH allow flag) is compiled in.
  mac-mgmt-daemon = pkgs.rustPlatform.buildRustPackage {
    pname = "mac-mgmt-daemon";
    version = "0.1.0";
    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;
    cargoBuildFlags = [ "-p" "mac-mgmt" "--no-default-features" "--features" "relay" ];
    doCheck = false;
  };

  # API-only server build — same override as the relay test.
  mac-mgmt-server-api = (pkgs.callPackage ../server/package.nix { }).overrideAttrs (old: {
    cargoBuildFlags = [ "-p" "mac-mgmt-server" "--no-default-features" "--features" "server-api-only" ];
    buildPhase = null;
    installPhase = null;
  });

  relayConfig = pkgs.writeText "relay.toml" ''
    listen_addr = "127.0.0.1:8080"
    ssh_port_min = 30000
    ssh_port_max = 30010
    server_api_url = "http://127.0.0.1:7378"
  '';

  # Pre-generate an SSH keypair so the daemon's authorized_keys file exists.
  # The test never actually opens an SSH session — it only needs the daemon
  # to register with the relay so the relay can fan out a metrics scrape.
  testKeyDir = pkgs.runCommand "test-ssh-keys" {} ''
    mkdir -p $out
    ${pkgs.openssh}/bin/ssh-keygen -t ed25519 -f $out/id_ed25519 -N "" -q
  '';

  daemonConfig = pkgs.writeText "daemon-config.toml" ''
    [metrics]
    port = 9396

    [server]
    url = "http://127.0.0.1:7378"
    token = "${syncToken}"

    [relay]
    url = "ws://127.0.0.1:8080"
  '';

  seedScript = pkgs.writeScript "seed-db.py" ''
    #!${pkgs.python3}/bin/python3
    import hashlib, subprocess, sys

    sync_hash = hashlib.sha256(b"${syncToken}").hexdigest()
    setting_hash = hashlib.sha256(b"${settingToken}").hexdigest()

    sql = f"""
    INSERT INTO customers (id, name) VALUES ('${customerId}', 'test-customer');
    INSERT INTO tokens (customer_id, token_hash, kind, label)
      VALUES ('${customerId}', '{sync_hash}', 'sync', 'test-sync');
    INSERT INTO tokens (customer_id, token_hash, kind, label)
      VALUES ('${customerId}', '{setting_hash}', 'setting', 'test-setting');
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
    imports = [ ../server/module.nix ];

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
        scheme = "http";
        # Inline bearer token (test-only secret) — using bearer_token_file
        # would fail prometheus' build-time config check because the file
        # does not yet exist on the build host.
        bearer_token = settingToken;
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
    assert json.loads(self_json)["customer_name"] == "test-customer"
    machine.log("Token validation verified via mac-mgmt-server")

    # Start the relay
    machine.execute(
        "mac-mgmt-relay -c ${relayConfig} >/tmp/relay.log 2>&1 &"
    )
    machine.wait_for_open_port(8080)
    machine.succeed("curl -sf http://127.0.0.1:8080/health")
    machine.log("Relay running on port 8080")

    # Sanity-check the federated /metrics endpoint reachable + auth-gated
    machine.fail("curl -sf http://127.0.0.1:8080/metrics")  # 401 — no token
    machine.succeed(
        "curl -sf -H 'Authorization: Bearer ${settingToken}' http://127.0.0.1:8080/metrics >/tmp/relay-metrics-empty.txt"
    )
    machine.log("Relay /metrics is reachable with bearer auth (no daemons yet)")

    # Wait for prometheus to come up
    machine.wait_for_unit("prometheus.service")
    machine.wait_for_open_port(9090)
    machine.log("Prometheus running on port 9090")

    # Start the daemon (no services feature, relay feature on).
    # Note: we deliberately do NOT enable remote SSH via the FIFO. The relay
    # client connects unconditionally when [relay].url is configured, and
    # metrics requests flow through regardless of the SSH allow flag.
    machine.succeed(
        "mkdir -p /root/.config/mac-mgmt/ssh && "
        "cp ${daemonConfig} /root/.config/mac-mgmt/config.toml && "
        "cp ${testKeyDir}/id_ed25519.pub /root/.config/mac-mgmt/ssh/authorized_keys"
    )
    machine.execute("mac-mgmt daemon >/tmp/daemon.log 2>&1 &")
    machine.log("Daemon kicked off")

    # Wait for the daemon to register with the relay (we go straight here
    # rather than wait_for_open_port on the daemon's local metrics server,
    # because that server binds to ::1 only and the registration is what
    # we actually care about for federation).
    relay_port = None
    instance_id = None
    for _ in range(60):
        try:
            tunnels_json = machine.succeed(
                "curl -sf -H 'Authorization: Bearer ${settingToken}' "
                "http://127.0.0.1:8080/api/tunnels"
            )
            tunnels = json.loads(tunnels_json)
            if len(tunnels) > 0:
                relay_port = tunnels[0]["ssh_port"]
                instance_id = tunnels[0]["instance_id"]
                break
        except Exception:
            pass
        time.sleep(1)
    if relay_port is None:
        machine.log("daemon did not register; dumping /tmp/daemon.log:")
        machine.log(machine.succeed("cat /tmp/daemon.log || true"))
    assert relay_port is not None, "daemon did not register with relay within 60s"
    machine.log(f"Daemon registered: instance_id={instance_id}")

    # Hit the federated /metrics directly to confirm the daemon scrape now
    # produces synthetic relay metrics carrying the federation labels.
    direct = machine.succeed(
        "curl -sf -H 'Authorization: Bearer ${settingToken}' http://127.0.0.1:8080/metrics"
    )
    machine.log("Federated /metrics body (first 1KB):")
    machine.log(direct[:1024])
    assert "mac_mgmt_relay_scrape_targets 1" in direct, \
        f"expected scrape_targets=1 in body:\n{direct}"
    assert "mac_mgmt_relay_scrape_up{" in direct, \
        f"expected scrape_up series in body:\n{direct}"
    assert f'instance_id="{instance_id}"' in direct, \
        f"expected instance_id label in body:\n{direct}"
    assert 'customer_id="${customerId}"' in direct, \
        f"expected customer_id label in body:\n{direct}"
    machine.log("Direct /metrics body carries federation labels")

    # Wait for Prometheus to scrape successfully at least once.
    target_up = False
    for _ in range(40):
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

    # Now query Prometheus for the synthetic metrics. Allow a couple of
    # additional scrape intervals so the value reflects the daemon being
    # registered (the very first scrape may have happened before the daemon
    # connected).
    targets_value = 0.0
    for _ in range(20):
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
        'customer_id="${customerId}"}'
    )
    up_value = 0.0
    series_labels: dict = {}
    for _ in range(20):
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
    assert series_labels.get("customer_id") == "${customerId}", \
        f"unexpected customer_id label: {series_labels}"
    assert "hostname" in series_labels, \
        f"hostname label missing: {series_labels}"
    machine.log(
        "mac_mgmt_relay_scrape_up=1 with federation labels: " + json.dumps(series_labels)
    )

    # And finally check the duration metric is being recorded.
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
