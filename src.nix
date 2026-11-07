# The source every Rust package here builds from: the tree minus what no build
# reads (CI, docs, deploy and test scripts, the flake's own Nix files), so a
# commit to those does not rebuild every package.
{ lib }:

lib.fileset.toSource {
  root = ./.;
  fileset = lib.fileset.difference ./. (lib.fileset.unions [
    ./.gitlab-ci.yml
    ./flake.nix
    ./flake.lock
    ./overlay.nix
    ./src.nix
    ./docker.nix
    ./otel-options.nix
    ./extra-hashes.nix
    ./AGENTS.md
    ./DESIGN.md
    ./README.md
    ./LICENSE.md
    ./tests
    ./fleet-deploy
    ./patches
    ./Procfile
    ./docker-compose.yml
    ./docker-compose.test.yml
    ./prebuild.sh
    ./xzar.sh
    ./docker-push.sh
    ./usb-release.sh
    ./build-usb.sh
    ./test.sh
    ./setup-configs.sh
    ./generate-model-catalog.sh
    ./start-daemon-directly.sh
    ./start-daemon-su.sh
    ./start-daemon-su-flame.sh
    ./start-usb-daemon-su.sh
  ]);
}
