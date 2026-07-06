{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.mmrcd;
  settingsFormat = pkgs.formats.toml { };
  # Non-secret settings rendered to the store; the token (and optional GitLab
  # token) are appended at runtime from files so they stay off the Nix store.
  baseConfig = settingsFormat.generate "mmrcd-base.toml" cfg.settings;
in
{
  options.services.mmrcd = {
    enable = lib.mkEnableOption "mmrcd incus orchestration daemon";

    package = lib.mkPackageOption pkgs "mmr-causality" { };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = ''
        Non-secret mmrcd configuration (rendered to TOML). Do NOT put the
        bearer `token` or `gitlab_token` here — use `tokenFile` /
        `gitlabTokenFile` so secrets stay out of the Nix store.
      '';
      example = lib.literalExpression ''
        {
          listen = "127.0.0.1:7390";
          incus_backend = "unix";
          incus_project_prefix = "mmrc";
          registry = "registry.plan.ai/plan-ai/mac-mgmt";
          gitlab_url = "https://git.plan.ai";
          gitlab_project = "plan-ai/mac-mgmt";
          default_ref = "trunk";
        }
      '';
    };

    tokenFile = lib.mkOption {
      type = lib.types.path;
      description = ''
        File containing the mmrcd bearer token (single line). mmrc must present
        the same value as `mmrcd_token`.
      '';
    };

    gitlabTokenFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Optional file containing a GitLab token (read_api) for resolving image
        tags from pipelines when the project is not publicly readable.
      '';
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "mmrcd";
      description = "User the daemon runs as. Added to the incus-admin group.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "mmrcd";
      description = "Group the daemon runs as.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open the mmrcd listen port. Usually left false (loopback).";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.${cfg.user} = {
      isSystemUser = true;
      group = cfg.group;
      home = "/var/lib/mmrcd";
      createHome = true;
      # Local incus socket access for spawning per-run projects.
      extraGroups = [ "incus-admin" ];
    };
    users.groups.${cfg.group} = { };

    services.mmrcd.settings = {
      listen = lib.mkDefault "127.0.0.1:7390";
      incus_backend = lib.mkDefault "unix";
      incus_project_prefix = lib.mkDefault "mmrc";
      run_dir = lib.mkDefault "/var/lib/mmrcd/runs";
    };

    systemd.services.mmrcd = {
      description = "mmrcd incus orchestration daemon";
      after = [ "network-online.target" "incus.service" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      # mmrcd shells out to the incus CLI for project/network lifecycle.
      path = [ pkgs.incus ];

      # Assemble the runtime config: non-secret base + token(s) from files.
      preStart = ''
        umask 077
        cp ${baseConfig} /run/mmrcd/config.toml
        printf 'token = "%s"\n' "$(cat ${cfg.tokenFile})" >> /run/mmrcd/config.toml
        ${lib.optionalString (cfg.gitlabTokenFile != null) ''
          printf 'gitlab_token = "%s"\n' "$(cat ${cfg.gitlabTokenFile})" >> /run/mmrcd/config.toml
        ''}
      '';

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --config /run/mmrcd/config.toml";
        Restart = "on-failure";
        RestartSec = 5;
        User = cfg.user;
        Group = cfg.group;
        SupplementaryGroups = [ "incus-admin" ];
        RuntimeDirectory = "mmrcd";
        RuntimeDirectoryMode = "0700";
        StateDirectory = "mmrcd";
        WorkingDirectory = "/var/lib/mmrcd";
        Environment = [ "RUST_LOG=mmrcd=info,mmr_causality=info" ];
        # Light hardening — the daemon must reach the incus unix socket and run
        # the incus CLI, so several sandboxes are intentionally left off.
        NoNewPrivileges = true;
        ProtectHome = true;
        PrivateTmp = true;
        RestrictRealtime = true;
      };
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts =
        let
          # listen "host:port" → port
          port = lib.toInt (lib.last (lib.splitString ":" (cfg.settings.listen or "127.0.0.1:7390")));
        in
        [ port ];
    };
  };
}
