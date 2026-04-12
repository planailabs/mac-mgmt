{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.mac-mgmt-server;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "config.toml" cfg.settings;
in
{
  options.services.mac-mgmt-server = {
    enable = lib.mkEnableOption "mac-mgmt server";

    package = lib.mkPackageOption pkgs "mac-mgmt-server" { };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = ''
        Configuration for mac-mgmt-server in Nix attribute set form.
        Rendered to config.toml.
      '';
      example = lib.literalExpression ''
        {
          database.url = "postgres://localhost/mac_mgmt";
          api.port = 7378;
          web.port = 7377;
          oidc = {
            client_id = "xxx.apps.googleusercontent.com";
            client_secret = "GOCSPX-xxx";
            redirect_uri = "https://example.com/auth/callback";
            allowed_domains = [ "example.com" ];
            cookie_secret = "generate-with-openssl-rand-hex-32";
          };
        }
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        File containing environment variables (e.g. secrets) for the service.
        Lines should be KEY=VALUE. Useful for passing secrets without
        putting them in the Nix store.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open the web and API ports in the firewall.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.mac-mgmt = {
      description = "mac-mgmt server";
      after = [ "network.target" "postgresql.service" ];
      wants = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      environment.CONFIG_PATH = configFile;
      path = [ config.nix.package ];

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 5;

        DynamicUser = true;
        StateDirectory = "mac-mgmt-server";

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

    services.mac-mgmt-server.settings = {
      database.url = "postgres:///mac-mgmt?host=/run/postgresql";
    };

    services.postgresql = {
      enable = true;

      ensureUsers = [{
        name = "mac-mgmt";
        ensureClauses.superuser = true;
        # ensurePermissions = { "DATABASE xzar" = "ALL PRIVILEGES"; };
      }];

      ensureDatabases = [ "mac-mgmt" ];
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts =
        let
          webPort = cfg.settings.web.port or 7377;
          apiPort = cfg.settings.api.port or 7378;
        in
        [ webPort apiPort ];
    };
  };
}
