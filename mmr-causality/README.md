# mmr-causality — `mmrc` + `mmrcd`

Unified chaos tooling for causing things to happen on a mac-mgmt cluster, in
**prod** (the mmr cluster) or an **antithesis** environment (fresh incus
containers, one isolated project per run). Workloads come from the
`antithesis-workloads` crate and assert eventual-consistency goals.

- **`mmrc`** — the CLI you drive.
- **`mmrcd`** — an orchestration daemon on an incus host that spins up the
  antithesis cluster (server(s), relays, nodes) from CI-built images.

```
mmrc ──HTTP(token)──▶ mmrcd ──incus──▶ [server │ relay │ daemon nodes]  (per-run project)
  └──────────── admin/setting/relay HTTP + relay tunnels ──────────────┘
```

## Build

```
cargo build -p mmr-causality --release
# binaries: target/release/mmrc  target/release/mmrcd
```

The Antithesis SDK is off by default (workloads run their real poll logic and
fail on timeout). To emit SDK assertions on the real platform, build the
workloads with `--features antithesis-workloads/antithesis`.

---

## mmrcd (orchestration daemon)

### Prerequisites
- An **incus** host with the `incus` CLI on `PATH` and either the local unix
  socket (default) or a remote REST endpoint reachable.
- Network access to the image registry (`registry.plan.ai/...`) and, for CI
  image resolution, to the GitLab API.

### Configure
```
mkdir -p ~/.config/mmrcd
cp mmr-causality/mmrcd.config.example.toml ~/.config/mmrcd/config.toml
# set `token` (openssl rand -hex 32) and, if the repo isn't public, `gitlab_token`.
```

### Run
```
mmrcd                       # uses ~/.config/mmrcd/config.toml
mmrcd --config /path.toml   # or an explicit path
RUST_LOG=mmrcd=debug,mmr_causality=debug mmrcd
```

mmrcd resolves images from the latest **successful** pipeline on `default_ref`
(trunk) and launches `test-mac-mgmt-{server,relay,daemon}:<short-sha>` from the
registry via incus. Each run gets its own incus project `mmrc-<run_id>`; teardown
deletes the project and everything in it.

> Dev without CI: set `pinned_tag = "latest"` in the config to skip pipeline
> resolution and use a manually-pushed tag.

---

## mmrc (CLI)

### Configure
```
mkdir -p ~/.config/mmrc
cp mmr-causality/mmrc.config.example.toml ~/.config/mmrc/config.toml
```
- `[env.antithesis].mmrcd_token` must equal mmrcd's `token`.
- `admin_token = "admin"` is the token the test-server image seeds; leave as-is.
- For prod, fill `[env.prod]` with the real server/relay URLs and admin token.

Environment is chosen by `MMRC_ENV` → `--env` → `default_env`.

### Emulator (the main antithesis loop)
Spins up a fresh cluster per round, picks workloads at random, verifies EC goals,
tears down. Reproducible from the printed seed.
```
MMRC_ENV=antithesis mmrc emulator run --rounds 5 --nodes 2
mmrc emulator run --seed 42 --rounds 1            # reproduce a specific run
mmrc emulator run --rounds 3 --keep-on-failure    # keep the incus project on failure
```

### Persistent cluster + single commands
`up` creates a run and prints host-reachable server/relay addresses; pass them to
subsequent commands (single antithesis commands don't own a run):
```
mmrc --env antithesis up --nodes 2
# → run r0001 up (image <sha>); server ... ; relay ...
mmrc --env antithesis --server-url <addr> --relay-url <addr> cluster ensure --ollama --memvault
mmrc --env antithesis --server-url <addr> --relay-url <addr> node ensure -n 2
mmrc --env antithesis --server-url <addr> --relay-url <addr> sse push --event sync_skills
mmrc --env antithesis --server-url <addr> --relay-url <addr> goals check --goal all
mmrc --env antithesis down --run-id r0001
```

### Against prod
Prod resolves URLs/tokens from config, so no `--server-url` is needed:
```
MMRC_ENV=prod mmrc goals check --goal all
MMRC_ENV=prod mmrc sse push --event request_assessment
MMRC_ENV=prod mmrc node probes
```
`node ensure` is rejected in prod (fleet nodes come from the runner; the chaos
API is disabled in production).

### Command map
```
mmrc [--env] [--config] [--cluster-id] [--server-url] [--relay-url]
  cluster ensure [--name] [--ollama] [--openclaw] [--memvault]
  node ensure -n N [--chaos] | heartbeat | probes
  relay <access|file-tunnel|shell-tunnel|logs|tcp-tunnel>
  sse push --event <name> [--instance <id>]
  skills <assign|remove|assert>
  memvault <doc|graph>
  goals check [--goal <services|probes|heartbeats|all>]
  images [--git-ref X]
  up [--nodes N] [--git-ref X] | down --run-id <id>
  emulator run [--rounds] [--seed] [--workloads-per-round] [--nodes] [--keep-on-failure] [--git-ref]
```

## Notes
- `images` shows the resolved registry tag + CI pipeline status via mmrcd.
- Chaos nodes require the server's chaos API, which is enabled only in the
  antithesis `test-mac-mgmt-server` image (`[chaos] enabled = true`), never in
  production.
