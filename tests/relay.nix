# NixOS integration test for the SSH relay.
#
# Tests the full relay flow:
#   1. Mock API server validates tokens
#   2. Relay starts and listens for daemon WebSocket connections
#   3. A Python "daemon" connects via WebSocket, receives a port
#   4. An SSH client connects to that port
#   5. The relay bridges TCP ↔ WebSocket
#   6. The daemon-side bridges WebSocket ↔ local OpenSSH
#   7. The SSH session runs a command and returns output
#
# Run with:  nix build .#checks.x86_64-linux.relay-integration -L
{
  pkgs,
  mac-mgmt-relay,
  ...
}:

let
  testPython = pkgs.python3.withPackages (ps: [ ps.websockets ]);

  # Pre-generate an SSH keypair for the test
  testKeyDir = pkgs.runCommand "test-ssh-keys" {} ''
    mkdir -p $out
    ${pkgs.openssh}/bin/ssh-keygen -t ed25519 -f $out/id_ed25519 -N "" -q
  '';

  mockApiScript = pkgs.writeScript "mock_api.py" ''
    #!${testPython}/bin/python3
    from http.server import HTTPServer, BaseHTTPRequestHandler
    import json

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == "/api/self":
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "customer_id": "550e8400-e29b-41d4-a716-446655440000",
                    "customer_name": "test-customer",
                    "token_kind": "setting",
                }).encode())
            else:
                self.send_response(404)
                self.end_headers()

        def log_message(self, format, *args):
            pass

    HTTPServer(("127.0.0.1", 7378), Handler).serve_forever()
  '';

  daemonSimScript = pkgs.writeScript "daemon_sim.py" ''
    #!${testPython}/bin/python3
    import asyncio
    import json
    import websockets

    RELAY_WS = "ws://127.0.0.1:8080"
    TOKEN = "test-token-abc123"
    INSTANCE_ID = "550e8400-test-daemon-0001"
    LOCAL_SSH_PORT = 2222

    async def bridge_session(session_id):
        uri = f"{RELAY_WS}/api/daemon/session/{session_id}"
        headers = {"Authorization": f"Bearer {TOKEN}"}
        async with websockets.connect(uri, additional_headers=headers) as ws:
            reader, writer = await asyncio.open_connection("127.0.0.1", LOCAL_SSH_PORT)

            async def ws_to_tcp():
                try:
                    async for msg in ws:
                        if isinstance(msg, bytes):
                            writer.write(msg)
                            await writer.drain()
                except Exception:
                    pass
                finally:
                    writer.close()

            async def tcp_to_ws():
                try:
                    while True:
                        data = await reader.read(8192)
                        if not data:
                            break
                        await ws.send(data)
                except Exception:
                    pass

            await asyncio.gather(ws_to_tcp(), tcp_to_ws())

    async def main():
        uri = f"{RELAY_WS}/api/daemon/register?instance_id={INSTANCE_ID}&agent_name=test-agent"
        headers = {"Authorization": f"Bearer {TOKEN}"}
        async with websockets.connect(uri, additional_headers=headers) as ws:
            msg = json.loads(await ws.recv())
            assert msg["type"] == "registered", f"unexpected: {msg}"
            port = msg["ssh_port"]
            print(f"REGISTERED port={port}", flush=True)

            with open("/tmp/relay_port", "w") as f:
                f.write(str(port))

            async for raw in ws:
                msg = json.loads(raw)
                if msg["type"] == "session_request":
                    sid = msg["session_id"]
                    print(f"SESSION {sid}", flush=True)
                    asyncio.create_task(bridge_session(sid))

    asyncio.run(main())
  '';

  relayConfig = pkgs.writeText "relay.toml" ''
    listen_addr = "127.0.0.1:8080"
    ssh_port_min = 30000
    ssh_port_max = 30010
    server_api_url = "http://127.0.0.1:7378"
  '';
in

pkgs.testers.nixosTest {
  name = "relay-integration";

  nodes.machine = { lib, ... }: {
    environment.systemPackages = [
      mac-mgmt-relay
      testPython
      pkgs.openssh
      pkgs.curl
    ];

    services.openssh = {
      enable = true;
      ports = [ 2222 ];
      settings = {
        PermitRootLogin = "yes";
      };
    };

    # Install the test public key for root
    users.users.root.openssh.authorizedKeys.keyFiles = [
      "${testKeyDir}/id_ed25519.pub"
    ];

    networking.firewall.enable = false;
  };

  testScript = ''
    import json
    import time

    machine.wait_for_unit("sshd.service")
    machine.wait_for_open_port(2222)

    # Copy test SSH key with correct permissions
    machine.succeed("cp ${testKeyDir}/id_ed25519 /tmp/test_key && chmod 600 /tmp/test_key")

    # Verify direct SSH works first (sanity check)
    machine.succeed(
        "ssh -p 2222 -i /tmp/test_key -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        "root@127.0.0.1 'echo DIRECT_OK'"
    )
    machine.log("Direct SSH works")

    # 1. Start mock API server
    machine.execute(
        "${mockApiScript} >/tmp/mock_api.log 2>&1 &"
    )
    machine.wait_for_open_port(7378)
    machine.log("Mock API server started on port 7378")

    # 2. Start the relay
    machine.execute(
        "mac-mgmt-relay -c ${relayConfig} >/tmp/relay.log 2>&1 &"
    )
    machine.wait_for_open_port(8080)
    machine.log("Relay started on port 8080")

    # Verify health endpoint
    machine.succeed("curl -sf http://127.0.0.1:8080/health")
    machine.log("Health check passed")

    # 3. Start daemon simulator
    machine.execute(
        "${daemonSimScript} >/tmp/daemon_sim.log 2>&1 &"
    )

    # Wait for daemon to register and write the port file
    machine.wait_for_file("/tmp/relay_port")
    relay_port = machine.succeed("cat /tmp/relay_port").strip()
    machine.log(f"Relay assigned SSH port: {relay_port}")

    # 4. Verify tunnel appears in the API
    tunnels_json = machine.succeed(
        "curl -sf -H 'Authorization: Bearer test-token-abc123' http://127.0.0.1:8080/api/tunnels"
    )
    tunnels = json.loads(tunnels_json)
    assert len(tunnels) == 1, f"expected 1 tunnel, got {len(tunnels)}: {tunnels}"
    assert tunnels[0]["instance_id"] == "550e8400-test-daemon-0001"
    assert tunnels[0]["customer_name"] == "test-customer"
    assert tunnels[0]["agent_name"] == "test-agent"
    assert tunnels[0]["ssh_port"] == int(relay_port)
    machine.log("Tunnel list API verified")

    # 5. SSH through the relay port
    machine.wait_for_open_port(int(relay_port))
    time.sleep(1)

    result = machine.succeed(
        f"ssh -p {relay_port} -i /tmp/test_key "
        f"-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        f"root@127.0.0.1 'echo RELAY_TEST_OK'"
    )
    assert "RELAY_TEST_OK" in result, f"SSH command output: {result}"
    machine.log("SSH through relay succeeded!")

    # 6. Verify a second session works
    result2 = machine.succeed(
        f"ssh -p {relay_port} -i /tmp/test_key "
        f"-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        f"root@127.0.0.1 'hostname'"
    )
    machine.log(f"Second SSH session returned: {result2.strip()}")

    machine.log("All relay integration tests passed!")
  '';
}
