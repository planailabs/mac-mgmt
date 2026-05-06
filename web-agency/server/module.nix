{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.web-agency-server;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "config.toml" cfg.settings;
in
{
  options.services.web-agency-server = {
    enable = lib.mkEnableOption "web-agency server";

    package = lib.mkPackageOption pkgs "web-agency-server" { };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = ''
        Configuration for web-agency-server in Nix attribute set form.
        Rendered to config.toml.
      '';
      example = lib.literalExpression ''
        {
          database.url = "postgres://localhost/web_agency";
          web.port = 7380;
          auth = {
            client_id = "xxx.apps.googleusercontent.com";
            client_secret = "GOCSPX-xxx";
            redirect_uri = "https://agency.example.com/auth/callback";
            allowed_domains = [ "example.com" ];
            cookie_secret = "generate-with-openssl-rand-hex-32";
          };
          secrets.encryption_key = "base64-encoded-32-byte-key";
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
      description = "Whether to open the web port in the firewall.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.web-agency = {
      description = "web-agency server";
      after = [ "network.target" "postgresql.service" ];
      wants = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      environment.CONFIG_PATH = configFile;

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 5;

        DynamicUser = true;
        StateDirectory = "web-agency-server";

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

    services.web-agency-server.settings = {
      database.url = "postgres:///web-agency?host=/run/postgresql";
    };

    services.postgresql = {
      enable = true;

      ensureUsers = [{
        name = "web-agency";
        ensureClauses.superuser = true;
      }];

      ensureDatabases = [ "web-agency" ];
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts = [ (cfg.settings.web.port or 7380) ];
    };
  };
}
