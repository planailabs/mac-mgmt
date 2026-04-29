# NixOS integration test for the SSE push notification system.
#
# Boots a single VM running PostgreSQL + mac-mgmt-server (API-only).
# A background curl streams SSE events; the test triggers API mutations
# and asserts the matching push events arrive over the SSE connection.
#
# Validates the full server-side flow:
#   API mutation → push::notify() → broadcast channel → SSE EventStream → client
#
# Run with:  nix build .#checks.x86_64-linux.sse-push -L
{ pkgs, ... }:

let
  syncToken = "test-sync-token-abc123";
  settingToken = "test-setting-token-abc123";
  clusterId = "550e8400-e29b-41d4-a716-446655440000";
  skillId = "a0000000-0000-0000-0000-000000000000";
  skillChannelId = "a0000000-0000-0000-0000-000000000001";
  mcpServerId = "b0000000-0000-0000-0000-000000000001";

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
    -- Seed a skill + channel and MCP server for assignment tests
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

  # Filters SSE "data:" lines from stdin (piped from curl -N) and writes
  # parsed JSON events to /tmp/sse-events.jsonl.
  sseFilter = pkgs.writeScript "sse-filter.py" ''
    #!${pkgs.python3}/bin/python3
    import json, sys
    with open("/tmp/sse-events.jsonl", "a") as out:
        for line in sys.stdin:
            line = line.strip()
            if line.startswith("data:"):
                data = line[len("data:"):].strip()
                try:
                    evt = json.loads(data)
                    out.write(json.dumps(evt) + "\n")
                    out.flush()
                except json.JSONDecodeError:
                    pass
  '';
in

pkgs.testers.nixosTest {
  name = "sse-push";

  nodes.machine = { lib, ... }: {
    imports = [ ../server/module.nix ];

    # Daemon's mcp_servers.rs calls `nix profile list --json`, which needs
    # the nix-command experimental feature — absent in the stock VM config.
    nix.settings.experimental-features = [ "nix-command" "flakes" ];

    environment.systemPackages = [
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

    def read_events():
        """Read all events collected so far by the SSE listener."""
        raw = machine.succeed("cat /tmp/sse-events.jsonl 2>/dev/null || true").strip()
        if not raw:
            return []
        return [json.loads(line) for line in raw.splitlines() if line.strip()]

    def wait_for_event(event_type, timeout=60):
        """Wait until an event with the given type appears in the log."""
        for _ in range(timeout * 2):
            evts = read_events()
            if any(e.get("type") == event_type for e in evts):
                return True
            time.sleep(0.5)
        evts = read_events()
        assert False, "event '{}' not received within {}s. Got: {}".format(
            event_type, timeout, evts
        )

    def clear_events():
        """Truncate the event log so the next wait_for_event starts clean."""
        machine.succeed("truncate -s 0 /tmp/sse-events.jsonl")

    def api(method, path, body=None):
        """Call the setting API and return stdout."""
        cmd = "curl -sf -X {} -H 'Authorization: Bearer ${settingToken}' ".format(method)
        if body is not None:
            cmd += "-H 'Content-Type: application/json' -d '{}' ".format(body)
        cmd += "http://127.0.0.1:7378" + path
        return machine.succeed(cmd)

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

    # ── Start SSE listener ─────────────────────────────────────────
    machine.succeed("touch /tmp/sse-events.jsonl")
    machine.execute(
        "curl -sN 'http://127.0.0.1:7378/api/events?token=${syncToken}' "
        "| ${sseFilter} >/tmp/sse-listener.log 2>&1 &"
    )
    time.sleep(5)
    machine.log("SSE listener started")

    # ── Test 1: Ping ───────────────────────────────────────────────
    machine.log("Waiting for initial ping...")
    wait_for_event("ping", timeout=90)
    machine.log("PASS: ping received")
    clear_events()

    # ── Test 2: SyncConfig on PUT (create) ─────────────────────────
    api("PUT", "/api/setting/config", "{}")
    wait_for_event("sync_config")
    machine.log("PASS: sync_config on PUT")
    clear_events()

    # ── Test 3: SyncConfig on PATCH ────────────────────────────────
    api("PATCH", "/api/setting/config",
        '{"section":"daemon","key":"health_interval","value":"2m"}')
    wait_for_event("sync_config")
    machine.log("PASS: sync_config on PATCH")
    clear_events()

    # ── Test 4: SyncSkills on skill assign ─────────────────────────
    api("POST", "/api/setting/skills",
        '{"skill_channel_id":"${skillChannelId}"}')
    wait_for_event("sync_skills")
    machine.log("PASS: sync_skills on assign")
    clear_events()

    # ── Test 5: SyncSkills on skill unassign ───────────────────────
    # Get the assignment ID first
    skills_json = api("GET", "/api/setting/skills")
    skills = json.loads(skills_json)
    assert len(skills) > 0, "expected at least one skill assignment"
    assign_id = skills[0]["id"]
    api("DELETE", "/api/setting/skills/" + assign_id)
    wait_for_event("sync_skills")
    machine.log("PASS: sync_skills on unassign")
    clear_events()

    # ── Test 6: SyncMcpServers on MCP assign ───────────────────────
    api("POST", "/api/setting/mcp-servers",
        '{"mcp_server_id":"${mcpServerId}"}')
    wait_for_event("sync_mcp_servers")
    machine.log("PASS: sync_mcp_servers on assign")
    clear_events()

    # ── Test 7: SyncMcpServers on MCP unassign ─────────────────────
    mcps_json = api("GET", "/api/setting/mcp-servers")
    mcps = json.loads(mcps_json)
    assert len(mcps) > 0, "expected at least one MCP assignment"
    mcp_assign_id = mcps[0]["id"]
    api("DELETE", "/api/setting/mcp-servers/" + mcp_assign_id)
    wait_for_event("sync_mcp_servers")
    machine.log("PASS: sync_mcp_servers on unassign")
    clear_events()

    # ── Test 8: SyncSshKeys on SSH key add ─────────────────────────
    api("POST", "/api/setting/ssh-keys",
        '{"public_key":"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBQUPSOi++vwIxBdPlcPn38HVeTaFXH/kqVCpOEgNzl7 test"}')
    wait_for_event("sync_ssh_keys")
    machine.log("PASS: sync_ssh_keys on add")
    clear_events()

    # ── Test 9: SyncSshKeys on SSH key remove ──────────────────────
    keys_json = api("GET", "/api/setting/ssh-keys")
    keys = json.loads(keys_json)
    assert len(keys) > 0, "expected at least one SSH key"
    key_id = keys[0]["id"]
    api("DELETE", "/api/setting/ssh-keys/" + key_id)
    wait_for_event("sync_ssh_keys")
    machine.log("PASS: sync_ssh_keys on remove")
    clear_events()

    # ── Test 10: Rapid-fire events all delivered ───────────────────
    for i in range(5):
        api("PUT", "/api/setting/config", "{}")
    # Allow ample time for all 5 events to propagate through the broadcast
    # channel and land in the listener log; a short sleep here is a common
    # source of flakiness under VM load.
    for _i in range(30):
        evts = read_events()
        if len([e for e in evts if e.get("type") == "sync_config"]) >= 5:
            break
        time.sleep(0.5)
    evts = read_events()
    config_evts = [e for e in evts if e.get("type") == "sync_config"]
    assert len(config_evts) == 5, \
        "expected 5 sync_config events, got {}: {}".format(len(config_evts), evts)
    machine.log("PASS: 5 rapid-fire sync_config events delivered")
    clear_events()

    # ── Test 11: Wrong token kind rejected ─────────────────────────
    rc, _ = machine.execute(
        "curl -sf 'http://127.0.0.1:7378/api/events?token=${settingToken}' "
        "--max-time 3 >/dev/null 2>&1"
    )
    assert rc != 0, "SSE should reject setting tokens"
    machine.log("PASS: setting token rejected for SSE")

    # ── Test 12: Invalid token rejected ────────────────────────────
    rc, _ = machine.execute(
        "curl -sf 'http://127.0.0.1:7378/api/events?token=bogus' "
        "--max-time 3 >/dev/null 2>&1"
    )
    assert rc != 0, "SSE should reject invalid tokens"
    machine.log("PASS: invalid token rejected for SSE")

    machine.log("All SSE push integration tests passed!")
  '';
}
