{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.mac-mgmt;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "config.toml" cfg.settings;
  stateDir = "/var/lib/mac-mgmt";
  configDir = "${stateDir}/.config/mac-mgmt";
  binPath = "${stateDir}/mac-mgmt";

  # Shared serviceConfig for both daemon and services-supervisor units.
  commonServiceConfig = {
    Restart = "always";
    RestartSec = 5;
    User = "mac-mgmt";
    Group = "mac-mgmt";
    StateDirectory = "mac-mgmt";
    WorkingDirectory = stateDir;
    RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" "AF_NETLINK" ];
  } // lib.optionalAttrs (cfg.environmentFile != null) {
    EnvironmentFile = cfg.environmentFile;
  };

  # Shared unit fields for both daemon and services-supervisor.
  commonUnitAttrs = {
    after = [
      "network-online.target"
      "mac-mgmt-download.service"
    ];
    wants = [ "network-online.target" ];
    requires = [ "mac-mgmt-download.service" ];
    wantedBy = [ "multi-user.target" ];
    restartTriggers = [ configFile cfg.version ];

    path = [
      config.nix.package
      pkgs.bashInteractive
      pkgs.coreutils
      pkgs.gitMinimal
    ];

    environment.HOME = stateDir;
  };
in
{
  options.services.mac-mgmt = {
    enable = lib.mkEnableOption "mac-mgmt daemon";

    serverUrl = lib.mkOption {
      type = lib.types.str;
      description = "Base URL of the mac-mgmt server (e.g. https://mgmt.example.com).";
      example = "https://mgmt.example.com";
    };

    version = lib.mkOption {
      type = lib.types.str;
      description = "Daemon version to download.";
      example = "0.1.5";
    };

    system = lib.mkOption {
      type = lib.types.str;
      default = builtins.currentSystem;
      description = "Nix system string (e.g. x86_64-linux). Defaults to the current system.";
    };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = ''
        Configuration for the mac-mgmt daemon in Nix attribute set form.
        Rendered to config.toml. The [server] section is managed
        automatically from serverUrl and the token in environmentFile.
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        File containing environment variables for the service.
        Must define MAC_MGMT_TOKEN with the server bearer token.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # Dedicated system user with stateDir as home so the daemon's
    # ~/.config/mac-mgmt resolution lands inside the state directory.
    users.users.mac-mgmt = {
      isSystemUser = true;
      group = "mac-mgmt";
      home = stateDir;
      createHome = true;
      shell = pkgs.bashInteractive;
    };
    users.groups.mac-mgmt = { };

    # Allow the mac-mgmt user to use nix (trusted for store access).
    nix.settings.trusted-users = [ "mac-mgmt" ];

    systemd.services.mac-mgmt-download = {
      description = "Download mac-mgmt daemon binary";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      unitConfig.ConditionPathExists = "!${binPath}";

      path = [ pkgs.curl pkgs.coreutils ];

      serviceConfig = {
        Type = "oneshot";
        User = "mac-mgmt";
        Group = "mac-mgmt";
        StateDirectory = "mac-mgmt";

        ExecStart = let
          url = "${cfg.serverUrl}/api/daemon-download/${cfg.version}/${cfg.system}";
        in pkgs.writeShellScript "mac-mgmt-download" ''
          set -euo pipefail
          mkdir -p "${configDir}"

          tmp="${binPath}.tmp.$$"
          curl -fsSL -o "$tmp" "${url}"
          chmod 0755 "$tmp"
          mv -f "$tmp" "${binPath}"
        '';
      };
    };

    systemd.services.mac-mgmt = commonUnitAttrs // {
      description = "mac-mgmt daemon";

      preStart = ''
        mkdir -p "${configDir}"
        install -m 0600 ${configFile} "${configDir}/config.toml"

        # Append [server] section from environment (token comes from
        # environmentFile and must not land in the nix store).
        if [ -n "''${MAC_MGMT_TOKEN:-}" ]; then
          printf '\n[server]\nurl = "%s"\ntoken = "%s"\n' \
            '${cfg.serverUrl}' "$MAC_MGMT_TOKEN" \
            >> "${configDir}/config.toml"
        fi

        # Ensure the user has a nix profile so the daemon can manage
        # packages via nix profile install/upgrade.
        if [ ! -e "${stateDir}/.nix-profile" ]; then
          nix profile install --profile "${stateDir}/.nix-profile" nixpkgs#hello 2>/dev/null || true
          nix profile remove --profile "${stateDir}/.nix-profile" '.*' 2>/dev/null || true
        fi
      '';

      serviceConfig = commonServiceConfig // {
        ExecStart = "${pkgs.bashInteractive}/bin/bash -lc 'exec ${binPath} daemon'";
      };
    };

    systemd.services.mac-mgmt-services = commonUnitAttrs // {
      description = "mac-mgmt managed-services supervisor";
      after = commonUnitAttrs.after ++ [ "mac-mgmt.service" ];
      restartIfChanged = false;

      serviceConfig = commonServiceConfig // {
        ExecStart = "${pkgs.bashInteractive}/bin/bash -lc 'exec ${binPath} services'";
      };
    };
  };
}
