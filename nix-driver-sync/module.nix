{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.nix-driver-sync;
  stateDir = "/var/lib/nix-driver-sync";
in
{
  options.services.nix-driver-sync = {
    enable = lib.mkEnableOption "NixOS opengl-driver sync to Incus containers";

    package = lib.mkPackageOption pkgs "nix-driver-sync" { };

    projects = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      description = "Incus project names to sync containers in.";
      example = [ "default" "gpu-workers" ];
    };

    interval = lib.mkOption {
      type = lib.types.str;
      default = "*-*-* *:00/5:00";
      description = "systemd calendar expression for the sync timer.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.nix-driver-sync = {
      description = "Sync opengl-driver into Incus containers";

      serviceConfig = {
        Type = "oneshot";
        ExecStart = let
          projectArgs = lib.concatMapStringsSep " " (p: "--project ${lib.escapeShellArg p}") cfg.projects;
        in "${lib.getExe cfg.package} ${projectArgs} --state-file ${stateDir}/state.json";
        StateDirectory = "nix-driver-sync";
      };

      path = [
        config.nix.package
        config.virtualisation.incus.package
        pkgs.coreutils
      ];
    };

    systemd.timers.nix-driver-sync = {
      description = "Timer for opengl-driver sync";
      wantedBy = [ "timers.target" ];

      timerConfig = {
        OnCalendar = cfg.interval;
        Persistent = true;
      };
    };
  };
}
