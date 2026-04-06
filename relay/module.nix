{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.mac-mgmt-relay;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "relay.toml" cfg.settings;
in
{
  options.services.mac-mgmt-relay = {
    enable = lib.mkEnableOption "mac-mgmt relay";

    package = lib.mkPackageOption pkgs "mac-mgmt-relay" { };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = ''
        Configuration for mac-mgmt-relay in Nix attribute set form.
        Rendered to relay.toml.
      '';
      example = lib.literalExpression ''
        {
          listen_addr = "0.0.0.0:8080";
          ssh_port_min = 30000;
          ssh_port_max = 40000;
          server_api_url = "http://localhost:7378";
        }
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        File containing environment variables (e.g. secrets) for the service.
        Lines should be KEY=VALUE.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open the listen and SSH port range in the firewall.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.mac-mgmt-relay = {
      description = "mac-mgmt SSH relay";
      after = [ "network.target" ];
      wants = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --config ${configFile}";
        Restart = "on-failure";
        RestartSec = 5;

        DynamicUser = true;
        StateDirectory = "mac-mgmt-relay";

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

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts =
        let
          listenPort = cfg.settings.listen_addr or 8080;
          sshMin = cfg.settings.ssh_port_min or 30000;
          sshMax = cfg.settings.ssh_port_max or 40000;
        in
        [ listenPort ] ++ lib.range sshMin sshMax;
    };
  };
}
