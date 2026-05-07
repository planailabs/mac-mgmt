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
  imports = [
    ../proxy/module.nix
  ];
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
    systemd.services.web-agency-server = {
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
        # Pin the primary group to the static shared group so:
        #  - the unit name never collides with the "web-agency" group
        #    that DynamicUser would otherwise auto-allocate, and
        #  - StateDirectory (/var/lib/web-agency-server, mode 0750) is
        #    group-readable by the proxy, which joins the same group via
        #    SupplementaryGroups.
        Group = "web-agency";
        StateDirectory = "web-agency-server";
        StateDirectoryMode = "0750";

        # Hardening
        CapabilityBoundingSet = "";
        LockPersonality = true;
        # V8 (Node.js / wrangler) needs JIT — cannot deny W^X.
        MemoryDenyWriteExecute = false;
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

    # Shared group so the proxy can read the server's internal token file.
    users.groups.web-agency = { };

    services.web-agency-server.settings = {
      # Peer auth: the server connects as its OS user (the dynamic user
      # for the systemd.services.web-agency-server unit, named
      # "web-agency-server"), so the postgres role must match.
      database.url = "postgres:///web-agency?host=/run/postgresql";
      proxy.internal_token_path = lib.mkDefault "/var/lib/web-agency-server/internal.token";
    };

    # Pre-configure proxy from the server's settings so values aren't duplicated.
    services.web-agency-proxy.settings.proxy = lib.mkIf (cfg.settings ? proxy) (
      let
        p = cfg.settings.proxy;
        port = cfg.settings.web.port or 7380;
      in
      {
        agency_domain = lib.mkIf (p ? agency_domain) (lib.mkDefault p.agency_domain);
        agency_upstream = lib.mkDefault "127.0.0.1:${toString port}";
        server_url = lib.mkDefault "http://127.0.0.1:${toString port}";
        internal_token_path = lib.mkIf (p ? internal_token_path) (lib.mkDefault p.internal_token_path);
        acme_email = lib.mkIf (p ? acme_email) (lib.mkDefault p.acme_email);
        http_addr = lib.mkIf (p ? http_addr) (lib.mkDefault p.http_addr);
        https_addr = lib.mkIf (p ? https_addr) (lib.mkDefault p.https_addr);
      }
    );

    services.postgresql = {
      enable = true;

      ensureUsers = [{
        name = "web-agency-server";
        ensureClauses.superuser = true;
      }];

      ensureDatabases = [ "web-agency" ];
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts = [ (cfg.settings.web.port or 7380) ];
    };
  };
}
