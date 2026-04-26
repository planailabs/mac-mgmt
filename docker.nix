{
  pkgs,
  mac-mgmt-server,
  mac-mgmt-relay,
  mac-mgmt-runner,
  mac-mgmt-relay-ssh,
  tag ? "latest",
}:

let
  cacert = pkgs.cacert;
  certEnv = "SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt";
in
{
  server = pkgs.dockerTools.buildLayeredImage {
    name = "mac-mgmt-server";
    inherit tag;
    contents = [ mac-mgmt-server cacert ];
    config = {
      Cmd = [ "${mac-mgmt-server}/bin/mac-mgmt-server" ];
      Env = [ certEnv ];
      ExposedPorts = {
        "7377/tcp" = {};
        "7378/tcp" = {};
      };
    };
  };

  relay = pkgs.dockerTools.buildLayeredImage {
    name = "mac-mgmt-relay";
    inherit tag;
    contents = [ mac-mgmt-relay cacert ];
    config = {
      Cmd = [ "${mac-mgmt-relay}/bin/mac-mgmt-relay" ];
      Env = [ certEnv ];
      ExposedPorts = {
        "8080/tcp" = {};
      };
    };
  };

  runner = pkgs.dockerTools.buildLayeredImage {
    name = "mac-mgmt-runner";
    inherit tag;
    contents = [ mac-mgmt-runner cacert ];
    config = {
      Cmd = [ "${mac-mgmt-runner}/bin/mac-mgmt-runner" ];
      Env = [ certEnv ];
    };
  };

  relay-ssh = pkgs.dockerTools.buildLayeredImage {
    name = "relay-ssh";
    inherit tag;
    contents = [ mac-mgmt-relay-ssh cacert ];
    config = {
      Cmd = [ "${mac-mgmt-relay-ssh}/bin/relay-ssh" ];
      Env = [ certEnv ];
    };
  };
}
