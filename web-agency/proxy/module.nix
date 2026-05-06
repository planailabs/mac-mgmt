{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.web-agency-proxy;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "proxy-config.toml" cfg.settings;
in
{
  options.services.web-agency-proxy = {
    enable = lib.mkEnableOption "web-agency reverse proxy";

    package = lib.mkPackageOption pkgs "web-agency-proxy" { };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = ''
        Configuration for web-agency-proxy in Nix attribute set form.
        Rendered to config.toml. Only the [proxy] section is used.
      '';
      example = lib.literalExpression ''
        {
          proxy = {
            agency_domain = "agency.example.com";
            agency_upstream = "127.0.0.1:7380";
            http_addr = "0.0.0.0:80";
            https_addr = "0.0.0.0:443";
            internal_token_path = "/var/lib/web-agency-server/internal.token";
            server_url = "http://127.0.0.1:7380";
          };
        }
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open HTTP/HTTPS ports in the firewall.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.web-agency-proxy = {
      description = "web-agency reverse proxy (Pingora)";
      after = [ "network.target" "web-agency.service" ];
      wants = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      environment.CONFIG_PATH = configFile;

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 5;

        DynamicUser = true;
        StateDirectory = "web-agency-proxy";
        SupplementaryGroups = [ "web-agency" ];

        # Needs to bind to ports 80/443
        AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" ];
        CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" ];

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
      };
    };

    # Fallback defaults when the proxy is used standalone (without the server module).
    services.web-agency-proxy.settings.proxy = {
      internal_token_path = lib.mkDefault "/var/lib/web-agency-server/internal.token";
      server_url = lib.mkDefault "http://127.0.0.1:7380";
      agency_upstream = lib.mkDefault "127.0.0.1:7380";
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts = [ 80 443 ];
    };
  };
}
