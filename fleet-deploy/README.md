# fleet-deploy

Deploy a mac-mgmt management server, a local xzar binary cache and a fleet
runner on a machine, driving instance creation on a remote Incus host.

Every script here is **generic**: all site specifics (host names, addresses,
ports, paths, tokens) come from `.env`, which is gitignored. Copy
`.env.example` to `.env` and fill it in.

## Layout

```
.env                 site config (gitignored; see .env.example)
.env.example         template with every key documented
lib.sh               sourced by every script: loads .env, derives paths
templates/           config templates with @PLACEHOLDER@ markers
setup.sh             one-time, idempotent: dirs, certs, DBs, configs, seed
publish-daemon.sh    build+publish the daemon binary into the local xzar
check.sh             are the three services up, and what does the DB hold
fleet-status.sh      services + runner cells + instance list
incus-ls.sh          instances in the fleet's Incus project
incus-projects.sh    every instance in every project (name-collision hunting)
instance-diag.sh     what one instance did with its cloud-init
instance-curl.sh     HTTP GET from inside an instance, plus its routes
v6-http.sh           compare the server's reachability from host and instance
xzar-pins.sh         pins in the local cache (daemon/<version>/<system>)
smoke-cloud-init.sh  end-to-end: mint cloud-init, fetch the daemon binary
seed.sh              admin token, organization, daemon_versions row
```

## Bringing one up

1. Build the binaries (in the mac-mgmt dev shell):

   ```
   cargo build --release -p mac-mgmt -p mac-mgmt-runner
   cd server && dx build --release
   ```

   Note: a bare `cargo` in this repo fails with a `plan-ai-design` lockfile
   collision once the nested submodule exists — run
   `python3 scripts/canonicalize-design-path.py memvault/crates/memvault-web/Cargo.toml`
   first (and restore that manifest afterwards).

2. Copy `.env.example` to `.env` and fill it in.

3. `./setup.sh` — creates the directories, the Incus client certificate (and
   trusts it on the host), both databases, the xzar signing key, renders the
   three configs from `templates/`, and seeds the server database.

4. `./publish-daemon.sh` — puts the daemon binary in the local xzar as
   `daemon/<version>/<system>`. Without this, cloud-init cannot resolve a
   version and answers 422.

5. Register the three services with reagent (see the notes on each),
   then `./check.sh` and `./fleet-status.sh`.

## Things that bit us

- **The daemon binary must be static (musl) for the instances.** A local
  `cargo build` / `dx build` / `nix build .#default` produces a glibc build
  whose ELF interpreter is a `/nix/store` path. A fleet instance is plain
  Ubuntu with no `/nix/store`, so the installed binary dies with
  `mac-mgmt: not found` even though the 142 MB file is right there. **CI does
  not publish that build**: `xzar.sh` cross-builds
  `--target x86_64-unknown-linux-musl` (with the cargo-zigbuild shim and an
  empty `libdl.a`) and uploads that. Pull the published artifact rather than
  rebuilding — `publish-daemon-from-ci.sh` reproduces CI's recipe and, as of
  this writing, fails on nameless `undefined symbol:` errors out of
  `zstd_sys` under `ld.lld`, so the local route is not yet reliable.
- **Instance names must be unique per bridge.** Two Incus projects sharing a
  bridge cannot reuse a name: the second instance refuses to start with
  `Instance DNS name "..." already used on network`. Set `INSTANCE_PREFIX`
  to something no other project on the bridge uses.
- **The server's `external_url` must be reachable from the instances.** On a
  bridge where guest-to-guest IPv4 is filtered, that means the bridge's
  global IPv6. The server binds IPv6 too (`server/src/api/mod.rs`), and the
  host's firewall must allow the fleet subnet to reach the API and cache
  ports.
- **`daemon_versions` needs a row** (or an active rollout/pin) or
  `POST /api/setting/cloud-init` returns 422 — see `seed.sh`.
- **`XZAR_SERVER_BIN` must be the unwrapped xzar-server.** The nixpkgs one
  (`/nix/store/*-xzar-server/bin/xzar-server`) is a wrapper that exports
  `XZAR_CONFIG`, and `XZAR_CONFIG` **overrides `--config`** — so it silently
  ignores the rendered `xzar/config.yaml` and serves upstream `xzar.plan.ai`
  instead (binds `::` rather than `127.0.0.1`, db role `xzar-server` which has
  no peer auth, storage `/var/lib/xzar-server/storage` which does not exist,
  and the `xzar.plan.ai` signing key the server does not trust). Its log looks
  like a clean startup — `Rocket has launched from http://[::]:17788` — while
  the health check keeps failing, so the service flaps forever. `lib.sh`
  rejects that path outright; the unwrapped binary is the last line of the
  wrapper script.
- **xzar derives its storage and database from `$HOME`** unless
  `XZAR_STORAGE` / `DATABASE_URL` are set; a service account has neither
  writable. The service definitions pass both.
