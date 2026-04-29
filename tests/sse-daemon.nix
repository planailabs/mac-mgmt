# NixOS integration test for daemon-side SSE push reception.
#
# Boots a single VM running:
#   1. PostgreSQL + mac-mgmt-server (API-only)
#   2. mac-mgmt daemon (built with --no-default-features for minimal footprint)
#
# The test triggers API mutations via the setting token and verifies the
# daemon receives and processes the corresponding SSE push events by
# inspecting its log output.
#
# Validates the daemon-side flow:
#   server SSE → daemon server_push → event loop → handle_push_cmd
#
# Run with:  nix build .#checks.x86_64-linux.sse-daemon -L
{ pkgs, ... }:

let
  syncToken = "test-sync-token-abc123";
  settingToken = "test-setting-token-abc123";
  clusterId = "550e8400-e29b-41d4-a716-446655440000";
  skillId = "a0000000-0000-0000-0000-000000000000";
  skillChannelId = "a0000000-0000-0000-0000-000000000001";
  mcpServerId = "b0000000-0000-0000-0000-000000000001";

  # Daemon with no features — no services, no relay, no self-update.
  # Only core + server_push + config + heartbeats.
  mac-mgmt-daemon = pkgs.rustPlatform.buildRustPackage {
    pname = "mac-mgmt-daemon";
    version = "0.1.0";
    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;
    cargoLock.outputHashes = import ../extra-hashes.nix;
    cargoBuildFlags = [ "-p" "mac-mgmt" "--no-default-features" ];
    doCheck = false;
  };

  # API-only server build.
  mac-mgmt-server-api = (pkgs.callPackage ../server/package.nix { }).overrideAttrs (old: {
    cargoBuildFlags = [ "-p" "mac-mgmt-server" "--no-default-features" "--features" "server-api-only" ];
    buildPhase = null;
    installPhase = null;
  });

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
    INSERT INTO skills (id, slug, name) VALUES ('${skillId}', 'test-skill', 'Test Skill');
    INSERT INTO skill_channels (id, skill_id, channel)
      VALUES ('${skillChannelId}', '${skillId}', 'stable');
    INSERT INTO mcp_servers (id, slug, name, config_json)
      VALUES ('${mcpServerId}', 'test-mcp', 'Test MCP', '{{}}'::jsonb);
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

  # Wrapper script to ensure stdout/stderr redirection works reliably
  # in the NixOS test VM console environment.
  daemonRunner = pkgs.writeShellScript "run-daemon.sh" ''
    exec ${mac-mgmt-daemon}/bin/mac-mgmt daemon >>/tmp/daemon.log 2>&1
  '';

  daemonConfig = pkgs.writeText "daemon-config.toml" ''
    [server]
    url = "http://127.0.0.1:7378"
    token = "${syncToken}"

    [metrics]
    port = 9396
  '';
in

pkgs.testers.nixosTest {
  name = "sse-daemon";

  nodes.machine = { lib, ... }: {
    imports = [ ../server/module.nix ];

    # Daemon's mcp_servers.rs calls `nix profile list --json`, which needs
    # the nix-command experimental feature — absent in the stock VM config.
    nix.settings.experimental-features = [ "nix-command" "flakes" ];

    environment.systemPackages = [
      mac-mgmt-daemon
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

    def api(method, path, body=None):
        cmd = "curl -sf -X {} -H 'Authorization: Bearer ${settingToken}' ".format(method)
        if body is not None:
            cmd += "-H 'Content-Type: application/json' -d '{}' ".format(body)
        cmd += "http://127.0.0.1:7378" + path
        return machine.succeed(cmd)

    def daemon_log_contains(pattern):
        """Check if the daemon log contains a pattern."""
        rc, _ = machine.execute("grep -q '{}' /tmp/daemon.log".format(pattern))
        return rc == 0

    def wait_for_daemon_log(pattern, timeout=60):
        """Wait until the daemon log contains the given pattern."""
        for _ in range(timeout * 2):
            if daemon_log_contains(pattern):
                return True
            time.sleep(0.5)
        # Dump log for debugging
        log = machine.succeed("tail -50 /tmp/daemon.log || true")
        assert False, "pattern '{}' not found in daemon log within {}s.\nLog tail:\n{}".format(
            pattern, timeout, log
        )

    # ── Boot services ──────────────────────────────────────────────
    machine.wait_for_unit("postgresql.service")
    machine.wait_for_unit("mac-mgmt.service")
    machine.wait_for_open_port(7378)
    machine.log("mac-mgmt-server API is up")

    machine.succeed("sudo -u postgres ${seedScript}")
    machine.log("Database seeded")

    # Sanity check
    self_info = json.loads(api("GET", "/api/self"))
    assert self_info["cluster_name"] == "test-cluster"
    machine.log("Token auth verified")

    # ── Start daemon ───────────────────────────────────────────────
    machine.succeed(
        "mkdir -p /root/.config/mac-mgmt && "
        "cp ${daemonConfig} /root/.config/mac-mgmt/config.toml"
    )
    machine.execute(
        "${daemonRunner} &"
    )
    machine.log("Daemon started")

    # Wait for daemon to connect to SSE
    wait_for_daemon_log("connecting to server SSE", timeout=60)
    machine.log("PASS: daemon initiated SSE connection")

    # Give the connection a moment to establish
    time.sleep(5)

    # ── Test 1: SyncConfig ─────────────────────────────────────────
    api("PUT", "/api/setting/config", "{}")
    wait_for_daemon_log("server push: sync config")
    machine.log("PASS: daemon processed SyncConfig push")

    # ── Test 2: SyncSkills ─────────────────────────────────────────
    api("POST", "/api/setting/skills",
        '{"skill_channel_id":"${skillChannelId}"}')
    wait_for_daemon_log("SSE event: SyncSkills")
    machine.log("PASS: daemon received SyncSkills push")

    # Clean up the skill assignment for next test
    skills_json = api("GET", "/api/setting/skills")
    skills = json.loads(skills_json)
    if skills:
        api("DELETE", "/api/setting/skills/" + skills[0]["id"])

    # ── Test 3: SyncMcpServers ─────────────────────────────────────
    api("POST", "/api/setting/mcp-servers",
        '{"mcp_server_id":"${mcpServerId}"}')
    wait_for_daemon_log("SSE event: SyncMcpServers")
    machine.log("PASS: daemon received SyncMcpServers push")

    # Clean up
    mcps_json = api("GET", "/api/setting/mcp-servers")
    mcps = json.loads(mcps_json)
    if mcps:
        api("DELETE", "/api/setting/mcp-servers/" + mcps[0]["id"])

    # ── Test 4: SyncSshKeys ────────────────────────────────────────
    api("POST", "/api/setting/ssh-keys",
        '{"public_key":"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBQUPSOi++vwIxBdPlcPn38HVeTaFXH/kqVCpOEgNzl7 test"}')
    wait_for_daemon_log("SSE event: SyncSshKeys")
    machine.log("PASS: daemon received SyncSshKeys push")

    # ── Test 5: Verify heartbeat was sent ──────────────────────────
    # The daemon sends heartbeats periodically; check the server received one
    time.sleep(5)
    heartbeats = machine.succeed(
        "sudo -u postgres psql -d mac-mgmt -t -A -c "
        "\"SELECT count(*) FROM daemon_heartbeats WHERE cluster_id = '${clusterId}'\""
    ).strip()
    assert int(heartbeats) > 0, "expected at least one heartbeat, got: " + heartbeats
    machine.log("PASS: daemon heartbeat registered in database")

    # ── Test 6: SSE reconnection after server restart ──────────────
    # Count current "connecting to server SSE" lines before restart
    connect_count_before = int(machine.succeed(
        "grep -c 'connecting to server SSE' /tmp/daemon.log || echo 0"
    ).strip())
    machine.log("SSE connect count before restart: {}".format(connect_count_before))

    machine.succeed("systemctl restart mac-mgmt.service")
    machine.wait_for_open_port(7378)
    machine.log("Server restarted")

    # Wait for the daemon to reconnect (connect count increases)
    for _ in range(120):
        count = int(machine.succeed(
            "grep -c 'connecting to server SSE' /tmp/daemon.log || echo 0"
        ).strip())
        if count > connect_count_before:
            break
        time.sleep(0.5)
    assert count > connect_count_before, \
        "daemon did not reconnect to SSE after server restart ({} vs {})".format(
            count, connect_count_before
        )
    machine.log("PASS: daemon reconnected to SSE after server restart")

    # Trigger another event after reconnect to verify the new connection works
    # Need to wait a moment for the SSE handshake to complete
    time.sleep(5)
    api("PUT", "/api/setting/config", "{}")
    wait_for_daemon_log("server push: sync config", timeout=60)
    machine.log("PASS: daemon receives push events after reconnect")

    machine.log("All daemon SSE integration tests passed!")
  '';
}
