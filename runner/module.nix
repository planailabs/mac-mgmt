{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.mac-mgmt-runner;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "mac-mgmt-runner.toml" cfg.settings;
in
{
  options.services.mac-mgmt-runner = {
    enable = lib.mkEnableOption "mac-mgmt fleet runner";

    package = lib.mkPackageOption pkgs "mac-mgmt-runner" { };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = ''
        Runner configuration as a Nix attribute set. Rendered to a TOML file
        and passed to the daemon via --config.

        Secrets (admin token, cloud API keys, Sentry DSN) should be supplied
        through `environmentFile` instead of being put in the Nix store.
        Use `<TOKEN>` placeholders in `settings` and the runner will read
        them from the environment via envsubst-style substitution at launch.
      '';
      example = lib.literalExpression ''
        {
          mgmt = {
            url = "https://mgmt.example.com";
            admin_token = "<ADMIN_TOKEN>";
            organization_id = "00000000-0000-0000-0000-000000000000";
          };
          incus = {
            url = "https://incus.example:8443";
            client_cert = "/var/lib/mac-mgmt-runner/incus-client.crt";
            client_key  = "/var/lib/mac-mgmt-runner/incus-client.key";
          };
          matrix.ollama_model = "smollm2:1.7b";
        }
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        File containing environment variables (secrets) for the service.
        Lines should be KEY=VALUE. Keeps admin tokens and API keys off the
        Nix store.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Open the runner's local HTTP API port (api.port) in the firewall.
        Usually left false — the API is meant to be reached only by the
        operator over SSH / a local socket.
      '';
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "mac-mgmt-runner";
      description = "System user the runner runs as.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "mac-mgmt-runner";
      description = "System group the runner runs as.";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.${cfg.user} = {
      isSystemUser = true;
      group = cfg.group;
      home = "/var/lib/mac-mgmt-runner";
      createHome = true;
    };
    users.groups.${cfg.group} = { };

    systemd.services.mac-mgmt-runner = {
      description = "mac-mgmt fleet runner";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --config ${configFile} daemon";
        Restart = "on-failure";
        RestartSec = 5;

        User = cfg.user;
        Group = cfg.group;
        StateDirectory = "mac-mgmt-runner";
        WorkingDirectory = "/var/lib/mac-mgmt-runner";

        # Hardening
        CapabilityBoundingSet = "";
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateTmp = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = true;
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectSystem = "strict";
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        SystemCallArchitectures = "native";
      } // lib.optionalAttrs (cfg.environmentFile != null) {
        EnvironmentFile = cfg.environmentFile;
      };
    };

    # Sensible defaults that are stable across upgrades — state path under
    # the systemd StateDirectory, HTTP API bound to loopback.
    services.mac-mgmt-runner.settings = {
      api.bind = lib.mkDefault "127.0.0.1";
      api.port = lib.mkDefault 9400;
      fleet.state_path = lib.mkDefault "/var/lib/mac-mgmt-runner/state.json";
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts =
        let
          port = cfg.settings.api.port or 9400;
        in
          [ port ];
    };
  };
}
