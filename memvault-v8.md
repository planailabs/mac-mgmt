# memvault v8 — Buckets, multi-bucket agents, cross-cluster share mailbox

**Status**: verified plan, anchored on actual codebase as of 2026-05-25
**Continues from**: memvault-v7.md (same design, simplified and codebase-verified)
**Milestone series**: B1–B8

---

## Summary

Introduce **Bucket** as a first-class scoping primitive. Buckets replace ad-hoc tag-as-scope conventions, support a lifecycle (private → attached → shared), and enable cross-cluster sharing via an approval mailbox.

All changes are additive. Old envelopes without a `bucket_id` belong to the cluster's default bucket.

---

## Design decisions verified against codebase

### Bucket membership is a first-class envelope field

`Signed<T>` (envelope.rs:13) gains `pub bucket_id: Option<BucketId>` with `#[serde(default)]`. The field is included in `SigningPayload` and covered by the signature. This is a wire-format change:

- **New envelopes** (with `bucket_id`) use `version: 2`. The signing payload includes the field.
- **Old envelopes** (without `bucket_id`) stay at `version: 1`. They deserialize with `bucket_id: None` via `#[serde(default)]`.
- **`Signed::sign()`** gains a `bucket_id: Option<BucketId>` parameter. When `Some`, version is set to 2; when `None`, version stays at 1 (byte-identical to today's output).
- **`Signed::verify()`** branches on version: v1 uses the old `SigningPayload` (no bucket field); v2 uses the new one (with bucket field). Both paths are tested.
- **Old nodes** that receive a v2 envelope see an unknown version and skip signature verification (accept-on-trust, log a warning). This is the same forward-compat pattern used for other version-gated features. Old nodes can still read the envelope's payload, tags, and CID — they just can't verify the signature until they upgrade.
- **`EnvelopeMeta`** (insert.rs:10) gets `bucket_id: Option<Vec<u8>>` extracted directly from the envelope field (not from tags).

No magic tags. Bucket is a proper signed field.

### Buckets are independent of clusters

A bucket can be created before any cluster exists. `memctl genesis` (memctl/src/lib.rs:302) creates directory structure and a cluster_id. Genesis is extended to:
- Accept an optional `--default-bucket <bucket-id>` flag pointing to a pre-existing bucket.
- If no flag is given, create a new bucket and bind it as the default.
- Either way, genesis writes a `BucketBinding { is_default: true }` to the store.

This supports the flow: create bucket → populate with data → run genesis → bucket is now the cluster's default with all its pre-existing content.

### Grant scopes extend additively

`Grant.scopes: Vec<TagPattern>` (grant.rs:35) stays as-is. A new `bucket_scopes: Vec<BucketId>` field is added with `#[serde(default)]` so existing grants deserialize unchanged. The verifier checks both fields (OR logic).

### Write methods use a WriteOptions struct

Rather than adding `bucket: Option<BucketId>` to every write method signature on `MemvaultClient` (client.rs:14), a `WriteOptions { bucket: Option<BucketId> }` struct wraps the optional params. Existing call sites pass `WriteOptions::default()`.

---

## Data shapes

### `BucketId` (new, in `memvault-core/src/ids.rs`)

Follows the `ClusterId`/`DocId` pattern exactly: `pub struct BucketId(pub [u8; 32])` with `random()`, `Display` (bs58).

### `BucketDecl` (new, in `memvault-doc/src/bucket.rs`)

A bucket is independent of any cluster. It can be created before genesis, populated with data, and then bound to a cluster later.

```rust
pub struct BucketDecl {
    pub bucket_id: BucketId,
    pub name: String,
    pub description: Option<String>,
    pub owner_agent: Option<AgentId>,
    pub default_visibility: Visibility,
    pub default_classification: Classification,
    pub created_ns: u64,
    pub private_to_peer: Option<PeerId>,
}
```

Note: no `is_default` field. Default-ness is a **cluster-level binding**, not a bucket property. A bucket doesn't know which cluster it belongs to — the `BUCKET_CLUSTER` table records that relationship, and the `CLUSTER_DEFAULT_BUCKET` table records which bucket is the cluster's default. This means:

- A bucket can exist before any cluster does (pre-genesis data staging).
- The same bucket could theoretically be bound to multiple clusters (shared origin).
- Genesis can accept an existing `BucketId` to bind as the new cluster's default.

### Bucket-cluster binding (new concept)

```rust
/// Records that a bucket is associated with a cluster.
/// Written at genesis (for the default bucket) or when a bucket is
/// attached to a cluster post-genesis.
pub struct BucketBinding {
    pub bucket_id: BucketId,
    pub cluster_id: ClusterId,
    pub bound_at_ns: u64,
    pub is_default: bool,           // true = this is the cluster's default bucket
}
```

### Bucket Op variants (appended to `Op` enum in `memvault-doc/src/op.rs`)

```rust
// Appended after EdgeUpdate — never inserted in the middle
BucketCreate   { decl: BucketDecl },
BucketBind     { bucket_id: BucketId, cluster_id: ClusterId, is_default: bool },
BucketRename   { bucket_id: BucketId, new_name: String },
BucketArchive  { bucket_id: BucketId, reason: String },
BucketAttach   { bucket_id: BucketId, attached_at_ns: u64 },
BucketGrantAgent { bucket_id: BucketId, agent: AgentId, actions: Vec<Action> },
BucketRevokeAgent { bucket_id: BucketId, agent: AgentId },
```

### Storage tables (new, in `memvault-store/src/tables.rs`)

Follow the existing `TableDefinition<&[u8], &[u8]>` pattern:

```rust
pub const BY_BUCKET: TableDefinition<&[u8], &[u8]> = TableDefinition::new("by_bucket");
pub const BUCKETS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("buckets");
/// bucket_id → cluster_id. Records which cluster a bucket is bound to.
/// A bucket with no entry here is unbound (pre-genesis or standalone).
pub const BUCKET_CLUSTER: TableDefinition<&[u8], &[u8]> = TableDefinition::new("bucket_cluster");
/// cluster_id → bucket_id. Records which bucket is a cluster's default.
pub const CLUSTER_DEFAULT_BUCKET: TableDefinition<&[u8], &[u8]> = TableDefinition::new("cluster_default_bucket");
pub const SHARE_INBOX: TableDefinition<&[u8], &[u8]> = TableDefinition::new("share_inbox");
pub const SHARE_OUTBOX: TableDefinition<&[u8], &[u8]> = TableDefinition::new("share_outbox");
pub const BUCKET_TRUST: TableDefinition<&[u8], &[u8]> = TableDefinition::new("bucket_trust");
```

### `EnvelopeMeta` extension (in `memvault-store/src/insert.rs`)

```rust
pub struct EnvelopeMeta {
    pub author: Vec<u8>,
    pub tags: Vec<(String, String)>,
    pub wall_ns: u64,
    pub causal: Vec<Vec<u8>>,
    pub provenance: Vec<Vec<u8>>,
    pub cluster_id: Option<Vec<u8>>,
    pub bucket_id: Option<Vec<u8>>,      // NEW — extracted from envelope.bucket_id field
}
```

`insert_envelope` and `reindex_block` write to `BY_BUCKET` when `bucket_id` is present. `clear_secondary_indexes` drains `BY_BUCKET` alongside the existing tables.

### Grant extension (in `memvault-auth/src/grant.rs`)

```rust
pub struct Grant {
    // ... existing fields unchanged ...
    pub scopes: Vec<TagPattern>,           // unchanged
    #[serde(default)]
    pub bucket_scopes: Vec<BucketId>,      // NEW — OR'd with tag scopes
}
```

The signing payload gains the field too, but old grants (empty `bucket_scopes`) produce identical signatures since `#[serde(default)]` deserializes as `[]` and DAG-CBOR encodes `[]` the same as absent-with-default.

### AgentEnrollment extension (in `memvault-auth/src/enrollment.rs`)

```rust
pub struct AgentEnrollment {
    // ... existing fields ...
    #[serde(default)]
    pub default_bucket: Option<BucketId>,  // NEW
}
```

### Cross-cluster share types (new, in `memvault-auth/src/share.rs`)

```rust
pub struct ShareProposal { ... }   // from_cluster, bucket_id, to_cluster, etc.
pub struct ShareReply { ... }      // decision, proposal_id, signature
pub struct BucketTrust { ... }     // bucket_id, from_cluster, to_cluster, actions, ttl
```

### Gossip extensions (in `memvault-net/src/gossip.rs` and `federation.rs`)

Append to `AdminAnnouncement`: `BucketCreated(Cid)`, `BucketAttached(Cid)`, `BucketArchived(Cid)`, `BucketTrustEstablished(Cid)`.

Append to `FederationAnnouncement`: `BucketTrustEstablished { trust_cid }`, `BucketTrustRevoked { revocation_cid }`. Extend `HeadAvailable` with `bucket_id: Option<Vec<u8>>`.

### Share protocol (new, in `memvault-net/src/share_proto.rs`)

`/ai-memvault/share/1.0` — request-response codec following `auth_proto`/`join_proto` pattern. Allowed on unauthenticated connections (like join).

---

## Phase plan

### B0 — Agent identity, enrollment, and HTTP auth

**Problem**: The auth types (`JoinToken`, `AgentEnrollment`, `MembershipAttestation`) exist in `memvault-auth` but nothing wires them together. There is no agent key storage, no enrollment ceremony, and no way for an agent (e.g. OpenClaw) to authenticate as itself. The HTTP API uses a single shared bearer token with no agent identity. `issue_token()` in `LocalClient` (local.rs:1085) is a placeholder returning `"mvjoin1:placeholder"`. Buckets depend on agent identity (ownership, default bucket, per-agent grants), so this must land first.

**Agent identity directory**: Each agent gets an identity directory at `~/.local/share/memvault/agents/<agent-id>/` containing:
- `private_key.pem` — Ed25519 private key (generated during enrollment)
- `attestation.cbor` — `Signed<MembershipAttestation>` from the cluster admin
- `enrollment.cbor` — `Signed<AgentEnrollment>` record
- `agent.json` — metadata: `{ agent_id, cluster_id, enrolled_at_ns, default_bucket }`

**Files**: `memvault-auth/src/token.rs`, `memvault-api/src/tokens.rs`, `memvault-api/src/local.rs`, `memvault-api/src/http.rs`, `memvault-api/src/agent_identity.rs` (new), `memvault-net/src/join_proto/handler.rs`, `memctl/src/lib.rs`, `plan-ai-memvault/src/lib.rs`

Tasks:

1. **Agent identity module** (`memvault-api/src/agent_identity.rs`, new):
   - `AgentIdentity { agent_id: AgentId, signing_key: SigningKey, verifying_key: VerifyingKey, attestation: MembershipAttestation, cluster_id: ClusterId }`.
   - `AgentIdentity::load(identity_dir: &Path) -> Result<Self>` — reads key + attestation from disk.
   - `AgentIdentity::generate_and_enroll(identity_dir: &Path, agent_id: &str, join_token: &str, cluster_admin_key: &VerifyingKey) -> Result<Self>` — generates Ed25519 keypair, decodes the join token, creates the enrollment request. The actual attestation comes back from the admin peer (step 3).
   - `AgentIdentity::sign_envelope<T>(&self, ...) -> Signed<T>` — signs with the agent's key.

2. **Implement `issue_token` properly** (`memvault-api/src/tokens.rs` and `local.rs`):
   - `issue_token` needs the cluster admin's `SigningKey`. `LocalClient` must be constructed with it (or with access to a keystore that holds it).
   - Build a real `JoinToken`, sign it, store the token block in the blockstore, return the `mvjoin1:` encoded string.
   - `list_tokens` scans the blockstore for token blocks (tag `kind:join-token`), cross-references `CONSUMED_TOKENS` for usage counts.

3. **Join protocol handler** (`memvault-net/src/join_proto/handler.rs`):
   - On receiving a `JoinRequest`: decode the `JoinToken` from `token_block`, verify signature against cluster admin keys, check `not_before`/`not_after`/`max_uses` against `CONSUMED_TOKENS`, record a `TokenConsumption`, create `AgentEnrollment` + `MembershipAttestation` signed by the admin key, return in `JoinResponse`.
   - The joining peer's `public_key` is carried in the `JoinRequest` (extend the struct with `pub agent_id: String` and `pub public_key: [u8; 32]`).

4. **Enrollment CLI** (`memctl`):
   - `memctl agent enroll --token "mvjoin1:..." --agent-id openclaw [--identity-dir PATH]`
     - Generates Ed25519 keypair.
     - Connects to a cluster peer (via `--url` or multiaddr).
     - Sends `JoinRequest` over `/join/1.0` with the token, agent_id, and public key.
     - Receives `JoinResponse` with attestation + enrollment.
     - Writes `private_key.pem`, `attestation.cbor`, `enrollment.cbor`, `agent.json` to identity dir.
   - `memctl agent list` — lists enrolled agents from the blockstore.
   - `memctl agent show <agent-id>` — shows enrollment details, grants, key status.
   - `memctl agent rotate-key <agent-id>` — triggers `AgentKeyRotation`.

5. **HTTP API agent auth** (`memvault-api/src/http.rs`):
   - Replace the single shared bearer token with agent-scoped tokens.
   - New daemon HTTP endpoint: `POST /api/auth/agent` accepts `{ attestation_block, signature }` (agent signs a challenge/nonce with its private key). Returns a scoped bearer token tied to that `AgentId` with a TTL.
   - `HttpApiClient::new()` gains `identity: Option<AgentIdentity>`. On construction, it calls `/api/auth/agent` to exchange its attestation for a scoped bearer token. Refreshes before expiry.
   - The daemon extracts `AgentId` from the scoped token on every request. Write operations record the agent as the author. Grant checks use the agent's grants.
   - **Backwards compat**: the old shared bearer token continues to work but maps to a synthetic `AgentId("anonymous")` with admin grants. Deprecation warning logged.

6. **MCP server agent identity** (`plan-ai-memvault/src/lib.rs`):
   - New CLI args: `--agent-id <name>` (env: `MEMVAULT_AGENT_ID`), `--identity-dir <path>` (env: `MEMVAULT_IDENTITY_DIR`, default: `~/.local/share/memvault/agents/<agent-id>/`).
   - On startup: loads `AgentIdentity::load(identity_dir)`. If not found and `--join-token` is provided, runs enrollment automatically.
   - Passes `AgentIdentity` to the `HttpClient` backend so all operations are authenticated as that agent.
   - The `MEMVAULT_DEFAULT_TAGS` env var pattern already exists; `MEMVAULT_AGENT_ID` follows the same convention.

7. **LocalClient gains agent context** (`memvault-api/src/local.rs`):
   - Constructor accepts `agent_identity: Option<AgentIdentity>` alongside `peer_id`.
   - Write operations use `agent_identity.agent_id` as the author when present (instead of bare `peer_id`).
   - Grant checks use the agent's grants when present.

8. **Token operations on HTTP** (`memvault-api/src/http.rs`):
   - Implement `issue_token` → `POST /api/tokens` (admin-only).
   - Implement `list_tokens` → `GET /api/tokens`.
   - Implement `revoke_token` → `DELETE /api/tokens/{cid}`.

9. **Daemon-driven enrollment** (`daemon/src/connectors/memvault_openclaw.rs` and new `daemon/src/agent_enroll.rs`):
   - The daemon already patches OpenClaw's config via the `MemvaultOpenClaw` connector (PreStart phase). It sets `MEMVAULT_URL` and `MEMVAULT_DEFAULT_TAGS` in the MCP server env (line 89). This is the natural place to also provision agent identity.
   - New helper `daemon/src/agent_enroll.rs`:
     - `ensure_agent_identity(agent_id: &str, identity_dir: &Path, store: &MemvaultStore, admin_key: &SigningKey) -> Result<AgentIdentity>`:
       1. If `identity_dir/private_key.pem` exists → load and return.
       2. Otherwise: generate Ed25519 keypair, issue a `JoinToken` internally (daemon holds admin key — no network round-trip needed), create `AgentEnrollment` + `MembershipAttestation`, write to blockstore, write identity dir.
     - This is a **local enrollment** — the daemon is both the admin issuing the token and the peer processing the join. No `/join/1.0` protocol call needed because the daemon has direct store access.
   - Extend `MemvaultOpenClaw::connect()`:
     - Call `ensure_agent_identity("openclaw", identity_dir, ...)` before patching config.
     - Add `MEMVAULT_AGENT_ID` and `MEMVAULT_IDENTITY_DIR` to the MCP server env block alongside existing vars.
   - Same pattern for `memvault_hermes.rs` (agent-id `hermes`).
   - The mcporter config (`~/.mcporter/plan-ai.json`) written by `sync_mcporter_config` also gets the identity env vars injected when the MCP server entry is `plan-ai-memvault`.

**Two enrollment paths** (daemon-automatic vs CLI-manual):

```
┌───────���─────────────────────────────────────────────────────────┐
│ Path A: Daemon-driven (automatic, no user action)               │
│                                                                 │
│ daemon starts → MemvaultOpenClaw connector (PreStart)           │
│   → ensure_agent_identity("openclaw", ~/.local/share/memvault/  │
│       agents/openclaw/, store, admin_key)                       │
│   → if identity missing: generate key, issue token internally,  │
│       enroll, write identity to disk                            │
│   → patch openclaw.json with MEMVAULT_AGENT_ID=openclaw         │
│       + MEMVAULT_IDENTITY_DIR=~/.local/share/memvault/agents/   │
│         openclaw/                                               │
│   → openclaw starts with MCP server authenticated as "openclaw" │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ Path B: CLI-manual (for external agents, headless setups)       │
│                                                                 │
│ admin$ memctl token-issue --role agent-host --label my-agent    │
│   → mvjoin1:3MFZWI3DE...                                       │
│                                                                 │
│ agent$ memctl agent enroll \                                    │
│     --token "mvjoin1:3MFZWI3DE..." \                            │
│     --agent-id my-agent \                                       │
│     --url http://localhost:8401                                  │
│   → connects to peer, redeems token via /join/1.0               │
│   → writes identity to ~/.local/share/memvault/agents/my-agent/ │
│                                                                 │
│ agent$ plan-ai-memvault --agent-id my-agent --url ...           │
│   → loads identity, authenticates via /api/auth/agent           │
└─────────────────────────────────────────────────────────────────┘
```

Path A is the common case for managed agents (OpenClaw, Hermes). Path B is for external or third-party agents that don't run under the daemon's supervisor.

**Acceptance**:
- `memctl token-issue` returns a real `mvjoin1:` token (not placeholder).
- `memctl agent enroll` with that token produces an identity directory with key + attestation.
- MCP server started with `--agent-id openclaw` authenticates to the daemon as openclaw.
- `HttpApiClient` with agent identity gets a scoped bearer token; write operations record openclaw as author.
- Old shared bearer token still works (backwards compat), maps to anonymous agent with deprecation warning.
- Token with `max_uses: 1` cannot be redeemed twice.
- Expired tokens are rejected. Revoked tokens are rejected.
- Agent key rotation via `memctl agent rotate-key` produces a new keypair; old attestation is superseded; new operations use the new key.
- **Daemon-driven**: daemon restart with memvault enabled auto-enrolls openclaw and hermes agents. Identity dirs are created, MCP server env vars are set. Agent can immediately write to memvault as its own identity.
- **Idempotent**: re-running `ensure_agent_identity` with an existing identity dir is a no-op (loads existing key, does not re-enroll).

---

### B1 — Bucket primitive and storage

**Files**: `memvault-core/src/ids.rs`, `memvault-core/src/envelope.rs`, `memvault-core/src/lib.rs`, `memvault-doc/src/bucket.rs` (new), `memvault-doc/src/op.rs`, `memvault-store/src/tables.rs`, `memvault-store/src/insert.rs`, `memvault-store/src/keys.rs`, `memvault-store/src/query.rs`, `memvault-api/src/types.rs`, `memvault-api/src/client.rs`, `memvault-api/src/local.rs`, `memctl/src/lib.rs`

Tasks:
1. Add `BucketId` to `ids.rs` (same pattern as `ClusterId`).
2. Add `bucket_id: Option<BucketId>` with `#[serde(default)]` to `Signed<T>` and `SigningPayload` in `envelope.rs`. Update `sign()` to accept the parameter and set `version: 2` when `Some`, `version: 1` when `None`. Update `verify()` to branch on version: v1 uses old payload layout, v2 includes `bucket_id`.
3. Add `BucketDecl` and `BucketBinding` structs in new `memvault-doc/src/bucket.rs`.
4. Append bucket `Op` variants to `op.rs` enum (after `EdgeUpdate`), including `BucketBind`.
5. Add `BY_BUCKET`, `BUCKETS`, `BUCKET_CLUSTER`, `CLUSTER_DEFAULT_BUCKET` tables to `tables.rs`.
6. Add `bucket_id: Option<Vec<u8>>` to `EnvelopeMeta`; extract from the envelope's `bucket_id` field in `insert_envelope`, `reindex_block`, `clear_secondary_indexes`.
7. Add `pack_bucket_key`/`unpack_bucket_cid` to `keys.rs`.
8. Add `query_by_bucket`, `list_buckets`, `get_default_bucket`, `get_bucket_cluster`, `bind_bucket` to store.
9. Add `BucketInfo` to `memvault-api/src/types.rs`. `BucketInfo` includes `cluster_id: Option<ClusterId>` (None if unbound).
10. Add `bucket_create`, `bucket_list`, `bucket_get`, `bucket_bind`, `bucket_rename` to `MemvaultClient` trait; implement in `LocalClient`. `bucket_create` does NOT require a cluster — it creates a standalone bucket in the store. `bucket_rename` writes a `BucketRename` op (LWW by lamport, same as other name-bearing CRDT ops).
11. Extend `memctl genesis` with `--default-bucket <bucket-id>` flag. If provided, binds that pre-existing bucket as the cluster's default. If omitted, creates a new bucket and binds it.
12. `memctl bucket new|list|show|bind|rename` subcommands. `bucket new` works without a cluster. `bucket bind <bucket-id> <cluster-id> [--default]` records the binding. `bucket rename <bucket-id> <new-name>` writes a rename op.
13. Tests:
    - Create a bucket with no cluster, write data into it, run genesis with `--default-bucket`, verify the bucket is now bound and all pre-existing data is queryable.
    - Create a bucket standalone, verify `BucketInfo.cluster_id` is `None`, bind it, verify it's now `Some`.
    - v1 envelopes (no bucket) still verify, v2 envelopes (with bucket) verify with new payload, old-format deserialization produces `bucket_id: None`.
    - Envelopes written to an unbound bucket still index correctly in `BY_BUCKET`.

**Acceptance**: Buckets exist independently of clusters. Pre-genesis bucket creation and population works. Genesis binds a bucket as the cluster default. `BY_BUCKET` index is populated and queryable. v1 and v2 envelope round-trips are both tested.

### B2 — Bucket-aware grants and agent enrollment

**Files**: `memvault-auth/src/grant.rs`, `memvault-auth/src/enrollment.rs`, `memvault-auth/src/verifier.rs`, `memvault-api/src/client.rs`, `memvault-api/src/local.rs`, `plan-ai-memvault/src/backend.rs`, `plan-ai-memvault/src/server.rs`, `memctl/src/lib.rs`

Tasks:
1. Add `bucket_scopes: Vec<BucketId>` with `#[serde(default)]` to `Grant` and `GrantSigningPayload`.
2. Add `default_bucket: Option<BucketId>` with `#[serde(default)]` to `AgentEnrollment`.
3. Extend `AuthVerifier` to check `bucket_scopes` (OR with `scopes`).
4. Add `WriteOptions { bucket: Option<BucketId> }` struct to `memvault-api/src/types.rs`.
5. Add `bucket: Option<BucketId>` parameter to write methods on `MemvaultClient`; `None` → agent's `default_bucket` → cluster default.
6. `bucket_grant_agent`, `bucket_set_default` on `MemvaultClient`.
7. MCP tools gain optional `bucket` parameter; `MEMVAULT_DEFAULT_BUCKET` env var.
8. `memctl bucket grant`, `bucket set-default`.
9. Property tests: old grants deserialize; bucket-scoped grants reject cross-bucket access.

**Acceptance**: Agent with grants on buckets A and B can write to both; writes to ungrated bucket C fail. Old grants work unchanged.

### B3 — Bucket attach, gossip filters, serve-side check

**Files**: `memvault-net/src/gossip.rs`, `memvault-net/src/visibility.rs`, `memvault-net/src/federation.rs`, `memvault-api/src/client.rs`, `memvault-api/src/local.rs`, `memctl/src/lib.rs`

Tasks:
1. `BucketAttach` op handling in `memvault-doc` apply path.
2. Gossip publish-side filter: skip heads for envelopes whose bucket has `private_to_peer = Some(_)`.
3. Serve-side filter: extend `VisibilityFilter::can_serve()` with private-bucket check and cross-cluster `BucketTrust` check (the `may_serve` function from the sync design).
4. `AdminAnnouncement::BucketCreated/Attached/Archived` variants.
5. `bucket_attach` on `MemvaultClient` — flips `private_to_peer` to `None`, triggers head re-publish.
6. `memctl bucket attach`.
7. Integration test: peer A keeps bucket private through 100 writes; peer B sees nothing; attach flips; B replicates within 5s. Defense-in-depth: direct bitswap WANT for private CID returns refusal.

**Acceptance**: Attach converges after partition heal. Re-attach is idempotent. Leaked CIDs still refused at serve time.

### B4 — Views, VFS, and search become bucket-aware

**Files**: `memvault-api/src/types.rs`, `memvault-api/src/vfs.rs`, `memvault-api/src/client.rs`, `memvault-api/src/local.rs`, `memvault-query/src/index/search.rs`, `plan-ai-memvault/src/server.rs`, `plan-ai-memvault/src/types.rs`, `plan-ai-memvault/src/backend.rs`, `plan-ai-memvault/src/local_backend.rs`, `memvault-web/src/api/search.rs`, `memvault-web/src/ui/pages/` (see Web UI section), `memvault-export/`

The search stack currently uses a simple in-memory TF-scoring engine (`TextIndex` in `search.rs`) backed by a `HashMap<String, IndexedEntry>` persisted as JSON. There's also an unused `TantivyIndex` (`tantivy_search.rs`) that only indexes docs (no entities or files). This phase promotes tantivy as the primary search engine, adds unified indexing (docs + entities + files), and wires bucket filtering as a native tantivy field.

```
MCP tools / Web UI API
    ↓
MemvaultClient trait / Backend trait
    ↓
TantivyIndex (disk-backed, BM25 scoring, memvault-query)
    ↓
Store indexes (redb: BY_BUCKET, BY_TAG, BY_TIME)
```

Tasks:

1. **Promote TantivyIndex to primary search engine** (`memvault-query/src/index/tantivy_search.rs`):

   Extend the existing schema (currently: `cid`, `doc_id`, `body`, `title`, `tags`, `wall_ns`) with:
   - `f_node_id: Field` (STRING | STORED) — `"doc:<hex>"`, `"entity:<hex>"`, `"file:<hex>"`. Replaces `doc_id` as the universal identifier.
   - `f_node_type: Field` (STRING | STORED) — `"doc"`, `"entity"`, `"file"`.
   - `f_bucket_id: Field` (STRING | STORED) — bs58-encoded `BucketId`, or empty string for unscoped. Used as a filter term in queries.
   - `f_label: Field` (TEXT | STORED) — human-readable name/title (boosted in queries).
   - Keep `f_body` (TEXT | STORED) for document body, entity properties, extracted file text.
   - Keep `f_tags` (STRING | STORED) for `scope:label` tag strings (multi-valued).
   - Keep `f_wall_ns` (u64, indexed | stored) for time-range filtering.
   - Drop `f_doc_id` (superseded by `f_node_id`).

   New methods:
   - `add_entity(node_id, kind, label, properties_text, tags, bucket_id, wall_ns)` — indexes entities (kind + props as body text, name/title as label).
   - `add_attachment(node_id, filename, mime_type, extracted_text, tags, bucket_id, wall_ns)` — indexes files (extracted text as body, filename as label).
   - Rename `add_document` → keeps the same role but gains `node_id` and `bucket_id` params.
   - `search_filtered(query, bucket_id, tag_filter, limit) -> Vec<TantivyHit>` — when `bucket_id` is set, prepends `bucket_id:<id> AND` to the tantivy query. When `tag_filter` is set, adds `tags:<scope:label> AND`. Tantivy handles the intersection natively.
   - `search_unified(query, bucket_id, limit) -> Vec<UnifiedHit>` — returns hits across all node types with `node_id`, `node_type`, `label`, `score`, `snippet`.
   - `retract(node_id)` — calls `remove()` by `node_id` term.

   The `TantivyIndex` is opened at a path under the memvault data dir (e.g. `<data_dir>/tantivy/`). On first open with an empty index, a full rebuild from the store is triggered (same as the current `populate_index` flow).

2. **Deprecate TextIndex** (`memvault-query/src/index/search.rs`):
   - Keep the file for one release cycle (callers may reference it).
   - `TextIndex` methods become thin wrappers that delegate to `TantivyIndex` during the transition.
   - Remove the `text_index.json` persistence — tantivy manages its own on-disk segments.
   - Eventually delete `search.rs` once all callers are migrated.

2. **MemvaultClient trait** (`memvault-api/src/client.rs`):
   - `search(&self, query: &str, limit: usize)` → `search(&self, query: &str, limit: usize, bucket: Option<&BucketId>)`.
   - `search_unified` gains the same parameter.
   - `list_docs` gains `bucket: Option<&BucketId>` (filters via `query_by_bucket` intersected with tag results).

3. **LocalClient** (`memvault-api/src/local.rs`):
   - `search()` (line 1016) passes bucket to `tantivy_idx.search_filtered()`.
   - `populate_index()` (line 118) switches from building in-memory `IndexedEntry` to calling `tantivy_idx.add_document/add_entity/add_attachment` for each item, then `tantivy_idx.commit()`.
   - The `index` field on `LocalClient` changes from `Arc<RwLock<TextIndex>>` to `Arc<RwLock<TantivyIndex>>`. Since `TantivyIndex` manages its own persistence, the JSON save/load cycle is removed.

4. **Backend trait + LocalBackend** (`plan-ai-memvault/src/backend.rs`, `local_backend.rs`):
   - `search()` gains `bucket: Option<&str>`.
   - `LocalBackend::search()` (line 97) passes bucket through to `self.client.search()`.

5. **MCP tools** (`plan-ai-memvault/src/server.rs`, `types.rs`):
   - `SearchParams` gains `bucket: Option<String>`. When set, only search within that bucket.
   - `ListParams` gains `bucket: Option<String>`.
   - `memvault_list_all` gains `bucket` parameter.
   - All read tools (`memvault_search`, `memvault_list`, `memvault_audit`, `memvault_traverse`, `memvault_vfs_ls`) gain optional `bucket` filter. Without it they search across all buckets the agent has read access to.

6. **Web UI search** (`memvault-web/src/api/search.rs`, `ui/pages/search.rs`):
   - `GET /api/v1/search` gains `bucket` query parameter.
   - Search page server function `search_docs()` passes bucket filter.
   - Search page UI: bucket filter dropdown above results.

7. **Views** (`memvault-api/src/types.rs`):
   - Add `bucket_id: Option<BucketId>` to `View` struct (old views deserialize with `None`).

8. **VFS** (`memvault-api/src/vfs.rs`):
   - `ensure_root` accepts optional bucket; per-bucket VFS roots tagged `(vfs:root, bucket:<bs58>)`.

9. **Web UI bucket filters** (see Web UI section):
   - Bucket filter dropdowns on notes/graph/files/audit/search pages.
   - Bucket switcher on VFS explorer.
   - Bucket field on view editor.

10. **Export** honors bucket scope.

**Acceptance**: Old views and VFS work unchanged. New bucket-scoped views hide cross-bucket entries. Search with `bucket` parameter returns only results from that bucket (tantivy handles the filter natively via term intersection). MCP `memvault_search` with `bucket` filters correctly. Tantivy index is rebuilt from store on first open with the new schema. Search results include docs, entities, and files (unified). BM25 scoring replaces the old TF heuristic.

### B5 — Share proposal protocol

**Files**: `memvault-auth/src/share.rs` (new), `memvault-net/src/share_proto.rs` (new), `memvault-store/src/tables.rs`, `memvault-store/src/insert.rs`, `memvault-api/src/client.rs`, `memvault-api/src/local.rs`, `memctl/src/lib.rs`

Tasks:
1. `ShareProposal`, `ShareReply`, `BucketTrust` types with sign/verify (following `Grant` pattern).
2. `/ai-memvault/share/1.0` request-response protocol (following `auth_proto`/`join_proto` pattern). Rate-limited per-cluster.
3. `SHARE_INBOX`, `SHARE_OUTBOX`, `BUCKET_TRUST` tables.
4. `share_propose`, `share_inbox`, `share_outbox` on `MemvaultClient`.
5. `memctl share propose|inbox|outbox`.
6. Two-cluster test: A proposes, B sees inbox, A sees outbox pending. Expired proposals auto-marked.

**Acceptance**: 100 concurrent proposals → consistent inbox. Rate-limit triggers `Refused` responses.

### B6 — Share approval, BucketTrust issuance, federated bucket reads

**Files**: `memvault-api/src/client.rs`, `memvault-auth/src/verifier.rs`, `memvault-net/src/visibility.rs`, `memvault-net/src/federation.rs`, `memvault-net/src/gossip.rs`, `memctl/src/lib.rs`

Tasks:
1. `share_decide` on `MemvaultClient`. On `Approve`: write `Signed<ShareReply>` + `Signed<BucketTrust>`, send back over `/share/1.0`, gossip `BucketTrustEstablished`, persist in `BUCKET_TRUST`.
2. Extend `can_serve` for cross-cluster: check `BucketTrust` for bucket reads.
3. Federation gossip filter: only publish `HeadAvailable` to clusters with matching `BucketTrust`.
4. Pending-bucket-discovery queue: when `BucketTrustEstablished` arrives for an unknown bucket, queue and retry fetching the `BucketDecl` via bitswap (5s, 30s, 5m backoff).
5. Double-decide race: deduplicate by `proposal_id`, pick earliest by `(lamport, signer_peer_id)`.
6. Trust revocation: evict from `BUCKET_TRUST` and verifier cache within next gossip tick.
7. `memctl share approve|reject`.
8. Tests: two-cluster approve flow, double-approve race, trust-before-decl convergence, revoke-mid-stream.

**Acceptance**: Approved bucket replicates cross-cluster. Revocation immediately stops reads. Attenuated approval works. Pending-discovery drains within 5 min.

### B7 — Share-agent MCP server and skill

**Files**: `plan-ai-memvault-share-agent/` (new crate), `.claude/skills/share-review/SKILL.md` (new), `memvault-web/` (see Web UI section for share inbox/outbox/detail pages)

Tasks:
1. New workspace crate `plan-ai-memvault-share-agent` with 6 tools: `share_inbox_list`, `share_inspect`, `share_check_classifications`, `share_approve`, `share_reject`, `share_staff_ping`.
2. Claude skill `.claude/skills/share-review/SKILL.md` for daily proposal triage.
3. Web UI: share inbox, outbox, and proposal detail pages (see Web UI section). Navbar gains Share Inbox/Outbox links.
4. Wire `share_staff_ping` to `mac-mgmt-healer` staff-ping pipeline.

**Acceptance**: LLM agent can triage proposals via MCP. Human can do the same from web UI.

### B8 — Quotas, GC, archive, operations

**Files**: `memvault-api/`, `memvault-doc/`, `memvault-web/`, `memctl/src/lib.rs`

Tasks:
1. Per-bucket caps on `bytes_stored`, `envelope_count`, `op_writes_per_minute` (extend existing `governor` integration).
2. `gc --bucket <id> --before TS` honors bucket scope.
3. `BucketArchive` op: flips archived flag, new writes refuse, reads continue.
4. Web UI: stats card and quota usage bar on bucket detail page, archive action button (see Web UI section).
5. `memctl bucket stats|archive|quota`.

**Acceptance**: `/chaos-test 10` with bucket lifecycle interleaved with restarts shows no inconsistency. Bucket GC doesn't affect other buckets.

---

## Web UI for buckets

The web UI follows the existing pattern: Dioxus fullstack with `#[server]` functions, `use_server_future()` for data fetching, `plan_ai_design` components (Card, DataTable, Button, Pill, PageHeader), and the Route enum in `app.rs`. Pages live under `memvault-web/src/ui/pages/`. The navbar in `navbar.rs` gets a "Buckets" entry.

Web UI work ships incrementally alongside backend phases. Each phase below notes which backend phase it requires.

### Routes (in `memvault-web/src/ui/app.rs`)

```rust
// Add to Route enum:
#[route("/buckets")]
BucketList {},
#[route("/buckets/:id")]
BucketDetail { id: String },
#[route("/buckets/:id/contents")]
BucketContents { id: String },
#[route("/share/inbox")]
ShareInbox {},
#[route("/share/outbox")]
ShareOutbox {},
#[route("/share/proposals/:id")]
ShareProposalDetail { id: String },
```

### Pages

#### `pages/buckets/list.rs` — Bucket list (ships with B1)

**Server functions**:
- `get_buckets()` → returns `Vec<BucketInfo>` (name, id, cluster binding, envelope count, created_ns, owner_agent, is_default, is_attached, archived).

**UI**:
- `PageHeader` with "Buckets" title and "New Bucket" button.
- Search bar filtering by name.
- `DataTable` with columns: Name, Status (Pill: unbound/private/attached/archived), Cluster (None or cluster_id truncated), Owner (agent or "cluster"), Items (envelope count), Created.
- Status pills: `unbound` (grey) = no cluster binding, `private` (yellow) = `private_to_peer` set, `attached` (green) = bound and gossiped, `archived` (red) = archived.
- Row click → `BucketDetail`.
- "New Bucket" button opens inline form or modal: name, description, visibility, classification. No cluster required.

#### `pages/buckets/detail.rs` — Bucket detail (ships with B1, extended in B2/B3/B8)

**Server functions**:
- `get_bucket(id)` → `BucketInfo` with full metadata.
- `get_bucket_agents(id)` → agents with grants on this bucket (B2).
- `get_bucket_stats(id)` → envelope count, byte size, last write timestamp (B8).
- `rename_bucket(id, new_name)` → writes `BucketRename` op.
- `bind_bucket(id, cluster_id, is_default)` → writes `BucketBind` op (B1).
- `attach_bucket(id)` → writes `BucketAttach` op (B3).
- `archive_bucket(id, reason)` → writes `BucketArchive` op (B8).

**UI**:
- `PageHeader` with bucket name (editable inline — triggers rename on blur/enter) and status Pill.
- Metadata `Card`: description, owner agent, default visibility/classification, created timestamp, cluster binding (or "Unbound" with a "Bind to Cluster" button).
- Actions `Card` (contextual based on state):
  - Unbound → "Bind to Cluster" button (cluster selector + default checkbox).
  - Private → "Attach to Cluster" button (makes visible to peers).
  - Attached → "Archive" button (with reason input).
  - All states → "Rename" (inline edit on the name).
- Agent access `Card` (B2): table of agents with their grant actions (Read/Write/Admin), with "Grant Agent" button.
- Stats `Card` (B8): envelope count, byte size, quota usage bar if quota is set.
- Share status `Card` (B6): list of `BucketTrust` records showing which clusters have access, with actions and TTL.
- "Browse Contents" link → `BucketContents`.

#### `pages/buckets/contents.rs` — Bucket contents browser (ships with B1)

**Server functions**:
- `get_bucket_envelopes(id, after_ns, limit)` → paginated list of envelopes in this bucket (uses `query_by_bucket`).

**UI**:
- `PageHeader` with bucket name as breadcrumb link back to detail.
- Same DataTable pattern as notes list but filtered to this bucket: columns for Type (doc/entity/edge/file), Title/Summary, Author, Time.
- Row click → existing NoteDetail/EntityDetail/FileDetail pages (the bucket is context, not a separate detail view).
- Pagination via "Load More" button.

#### `pages/share/inbox.rs` — Share inbox (ships with B7)

**Server functions**:
- `get_share_inbox(only_pending)` → `Vec<ShareProposal>` with status.
- `approve_proposal(cid, attenuated_actions, ttl)` → `share_decide(Approve)`.
- `reject_proposal(cid, reason)` → `share_decide(Reject)`.

**UI**:
- `PageHeader` with "Share Inbox" title and pending count badge.
- `DataTable` with columns: From Cluster, Bucket Name, Purpose, Proposed Actions, Status (Pill: pending/approved/rejected/expired), Received.
- Row click → `ShareProposalDetail`.
- Inline approve/reject buttons on pending proposals (with confirmation modal for approve, reason input for reject).
- `provisional_first_contact` proposals highlighted with a warning banner.

#### `pages/share/outbox.rs` — Share outbox (ships with B7)

**Server functions**:
- `get_share_outbox()` → `Vec<(ShareProposal, ShareStatus)>`.

**UI**:
- `DataTable` with columns: To Cluster, Bucket Name, Purpose, Status (pending/approved/rejected), Sent.
- Row click → `ShareProposalDetail` (read-only view of the proposal + reply if received).

#### `pages/share/detail.rs` — Proposal detail (ships with B7)

**UI**:
- Full proposal metadata: from/to cluster, bucket, proposer admin key, purpose, proposed actions, TTL.
- Signature chain visualization (verified/unverified Pill).
- For inbox proposals: approve/reject action buttons.
- For approved proposals: resulting `BucketTrust` details (actions granted, expiry).
- First-contact warning banner if applicable.

### Existing pages gain bucket awareness (ships with B4)

- **Notes list** (`pages/notes/list.rs`): add bucket filter dropdown in the toolbar. Default: "All buckets". When a bucket is selected, only notes from that bucket are shown.
- **Graph explorer** (`pages/graph/explorer.rs`): same bucket filter dropdown.
- **File explorer** (`pages/files/explorer.rs`): same bucket filter dropdown.
- **VFS explorer** (`pages/vfs/explorer.rs`): bucket switcher in the breadcrumb area. Each bucket has its own VFS root.
- **Views page** (`pages/views.rs`): views gain a bucket field. The view editor shows a bucket selector.
- **Audit log** (`pages/audit.rs`): bucket filter for scoping audit queries.

### Navbar update (ships with B1)

In `navbar.rs`, add under a "Storage" or "Data" section:
```
Buckets     → /buckets
```

After B7, add under an "Operations" section:
```
Share Inbox  → /share/inbox
Share Outbox → /share/outbox
```

### MCP tools gain rename (ships with B1)

In `plan-ai-memvault`, add `memvault_bucket_rename` tool alongside the other bucket tools. Parameters: `bucket_id: String`, `new_name: String`.

---

## Sync and consistency model

Unchanged from v7 design. Three guarantees:

1. **Strong eventual consistency** on document state (Loro CRDT).
2. **Causal consistency** on op ordering (via `causal: Vec<Cid>` + Lamport).
3. **Monotonic append-only audit log** (CIDs are immutable once committed).

The **serve-side check** (`can_serve`/`may_serve` in `visibility.rs`) is the security boundary. Gossip filtering is a performance optimization only. Every new bitswap serve path must consult it.

Key sync paths exercised by buckets:
- **Private**: gossip publish-side skips; serve-side refuses.
- **Attached**: `AdminAnnouncement::BucketAttached` triggers head re-publish; peers fetch via bitswap.
- **Share proposal**: synchronous request-response over `/share/1.0` (delivery guarantee).
- **Shared bucket data**: `FederationAnnouncement::HeadAvailable` with `bucket_id`; serve-side checks `BucketTrust`.

See v7 plan for the full race resolution table.

---

## Execution order and parallelism

B0 (agent identity) is the true foundation — buckets need agent identity for ownership, default bucket, and grants. B1 (bucket storage) depends on B0.

```
B0 ──► B1 ──┬──► B2 ──────────┐
             │                  │
             ├──► B3 ──────────┼──► B6 ──► B7
             │                  │
             ├──► B5 ──────────┘
             │
             ├──► B4
             │
             └──► B8
```

- **B0** lands first (agent identity, enrollment, HTTP auth).
- **B1 depends on B0** (bucket ownership needs agent identity).
- **B2 + B3 + B5 can run in parallel after B1** (grants, gossip filters, share protocol).
- **B4 and B8 depend only on B1** (views, quotas).
- **B6 depends on B2 + B3 + B5** (needs grants, serve filtering, and share protocol).
- **B7 depends on B6** (needs working share flow).

Optimal path for a 2-engineer team: Engineer A does B0 → B1 → B3 → B4. Engineer B starts B5 after B1 lands, then B2, then both converge on B6 → B7 → B8.

---

## What this plan does NOT do

- No vector embeddings (v9).
- No replacement of Views (complementary to buckets).
- No new ALPN beyond `/share/1.0`.
- No auto-approval policy (every decision is recorded).
- No linearizability or cross-bucket transactions.
- No automatic recovery from BucketTrust revocation (B8 metric only).

---

## Caveats

1. **Agent enrollment is a prerequisite for everything**. Until B0 lands, all memvault operations are effectively anonymous. Existing deployments need a migration path: generate identities for existing agents, backfill enrollments, then cut over to agent-scoped auth.
2. **Envelope version bump** (v1 → v2) means old nodes cannot verify v2 signatures. Old nodes must be updated to at least accept-on-trust for v2 envelopes, or they'll reject bucket-bearing data. Plan a coordinated rollout: ship v2-aware verification (that accepts both versions) first, then start emitting v2 envelopes.
3. **Write method refactor** is wide (memctl, plan-ai-memvault, memvault-web, memvault-export, tests). The `WriteOptions` struct minimizes signature churn but every call site still needs updating.
4. **BucketTrust is per-pair** (from → to, per bucket). Three target clusters = three trusts.
5. **Old envelopes** (no `bucket_id`) land in the cluster's default bucket on first indexing. `memctl repair-index` re-derives `BY_BUCKET`. If no cluster exists yet (pre-genesis store), old envelopes have `bucket_id: None` and don't appear in any bucket index until a default bucket is bound.
6. **Share-agent keys** should rotate on existing `AgentKeyRotation` cadence with minimal grants.
7. **First-contact identity** assumes out-of-band key verification. Multi-sig/TOFU policies can layer on later.
8. **Firewall asymmetry**: reply leg requires outbound dialing. The relay subsystem handles this for daemons; share protocol can piggyback on it.
9. **Partial replicas on revocation**: blocks already fetched are kept (audit monotonicity). Operators run `memctl bucket purge` explicitly if needed.
10. **Pending-discovery queue** is bounded (default 1000) to prevent unbounded growth from permanently-offline proposers.
