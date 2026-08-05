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
  # Read after Environment=, so a secret here wins over the store-visible
  # otlpHeaders in relay.toml.
  envFiles = lib.filter (f: f != null) [ cfg.environmentFile cfg.otlpHeadersFile ];
in
{
  options.services.mac-mgmt-relay = (import ../otel-options.nix { inherit lib; }) // {
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
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" "AF_NETLINK" ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        SystemCallArchitectures = "native";
      } // lib.optionalAttrs (envFiles != [ ]) {
        EnvironmentFile = envFiles;
      };
    };

    services.mac-mgmt-relay.settings.opentelemetry = lib.mkIf (cfg.otlpEndpoint != null) {
      server = cfg.otlpEndpoint;
      headers = lib.mapAttrsToList (key: value: { inherit key value; }) cfg.otlpHeaders;
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts =
        let
          sshMin = cfg.settings.ssh_port_min or 30000;
          sshMax = cfg.settings.ssh_port_max or 40000;
        in
          lib.range sshMin sshMax;
    };
  };
}
