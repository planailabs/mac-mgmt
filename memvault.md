# AI Memvault — Build Plan (v6)

A standalone p2p memory store for agents (openclaw and others). Many cooperating clusters, each with many peers, libp2p-native, content-addressed, multi-writer convergent. First-class history, audit, retraction, classification, PII handling, federation, knowledge summarization, token-based admission, key rotation, and a structural web interface.

---

## 0. What changed from v5

Two operational features added:

- **Token-based join system.** Admins issue short-lived `JoinToken` blocks; new peers redeem them via `/ai-memvault/join/1.0` to receive a `MembershipAttestation`. Avoids manually crafting attestation files for every new peer. Phase 1 deliverable.
- **Key rotation.** Admin keys and agent keys can be rotated via signed `KeyRotation` blocks with overlap windows. Cluster identity (`ClusterId`) is now a stable random value separate from any specific key, so admin-set evolution doesn't change the cluster's identity. Basic rotation primitives in Phase 1; full rotation API in Phase 3; advanced lifecycle (multi-sig, automated) in Phase 8.

Everything else from v5 stays.

---

## Table of contents

1. What this system is for
2. Why these crates (and not others)
3. Architectural commitments
4. Threat model
5. Data shapes
6. Layered architecture
7. Federation
8. Classification, PII, and egress control
9. Token-based admission
10. Key rotation
11. Web interface
12. Workspace structure
13. Cargo dependencies
14. Wire protocols
15. Filesystem layout
16. Phase plan
17. Open decisions
18. Risks
19. First-week tasks

---

## 1. What this system is for

Persistent, shared memory for AI agents (openclaw and others) across:

- **Markdown notes with frontmatter** and **knowledge graphs** (entities + typed edges), equal weight.
- **Audit, history, retraction** as inherent properties of the data model.
- **Summaries** generated on demand, with provenance back to sources.
- **Cross-cluster sharing** with cooperative trust and per-cluster grants.
- **Classification + PII gating** for egress to external destinations.
- **Token-based admission** so onboarding new peers is operationally simple.
- **Key rotation** so admin and agent keys can be rotated without a cluster rebuild.
- **Web inspection and operation** via a Dioxus webapp; consumers bring CSS.

The data plane is fully p2p: no central server. Every peer holds the blocks it has fetched, reconciles via bitswap, propagates new heads via gossipsub.

---

## 2. Why these crates (and not others)

| Layer | Crate | Why |
|---|---|---|
| Transport | `libp2p` (existing) | already deployed |
| IPLD | `ipld-core` + `serde_ipld_dagcbor` | post-libipld standard, deterministic codec |
| CIDs | `cid` + `multihash-codetable` | flexible hash; BLAKE3 default |
| Block exchange | `beetswap` (celestiaorg) | actively maintained libp2p-native bitswap |
| Blockstore trait | `blockstore` (eigerco) | what beetswap consumes |
| Persistence | `redb` | embedded, ACID, no native deps |
| CRDT | `loro` | rich-text-aware, fast, supports text/map/list |
| Full-text | `tantivy` | the Rust standard |
| RPC | `tarpc` | tokio-native |
| HTTP | `axum` | tokio-native |
| Web frontend | `dioxus` | Rust-native UI; compiles to WASM |
| PII regex | `regex` + `aho-corasick` | fast pattern matching |
| Token encoding | `bs58` or `base32` | compact human-shareable token strings |

**Did not choose:** `rust-ipfs` (archived), `ipfs-embed` (abandoned), `libipld` (deprecated), `iroh` (not libp2p-based).

**Rejected at data layer:** DAG-JOSE (interop we don't need), UCAN (spec pre-1.0; hand-rolled grants are sufficient).

---

## 3. Architectural commitments

### Topology

Many clusters, many peers per cluster. Each cluster has a curated set of admin public keys (evolved via signed rotation blocks). Federation is opt-in via `ClusterTrust` entries.

### Cluster identity

`ClusterId` is a stable random 32-byte value generated at cluster genesis, **not** derived from any specific key. This decouples cluster identity from admin key membership: rotating admins does not change the `ClusterId`. The active admin key set is separate state, evolved via `Signed<AdminKeyRotation>` blocks.

### Transport

Existing libp2p stack. **One** `Swarm` per node. **No PSK.** Cluster and federation admission both use the same application-layer auth handshake on `/ai-memvault/auth/1.0`. New peers join via `/ai-memvault/join/1.0` (see §9).

### Authentication (peer ↔ peer)

Every connection completes the auth handshake before any other application protocol is allowed. The connection's verified libp2p PeerId is bound to a `MembershipAttestation` whose `member` field equals that PeerId. The `AuthVerifier` accepts attestations signed by:

1. Any **currently valid** admin key of our cluster (intra admission).
2. Any currently valid admin key of any cluster in `ClusterTrust` (federation admission).

"Currently valid" accounts for active rotation overlap windows — see §10.

### Authorization (agent ↔ store)

Distinct from peer auth. Each agent has its own keypair. `AgentEnrollment` blocks bind an `AgentId` to a public key with TTL and initial grants. `Grant` blocks signed by admins reference tag patterns and actions: `Read | Write | Admin | Egress`.

### Token-based admission

Admins issue `JoinToken` blocks (signed, time-limited, max-uses-bounded). New peers redeem tokens via `/ai-memvault/join/1.0` to receive a `MembershipAttestation` bound to their PeerId. Token consumption is gossipsub-tracked and recorded in a redb table to prevent re-redemption. See §9.

### Key rotation

Admin keys and agent keys rotate via `Signed<KeyRotation>` blocks signed by both old and new key (proves old authorizes, new possesses). Rotation has an overlap window during which both keys are valid. After the window, only the new key. Cluster identity (`ClusterId`) is unaffected. See §10.

### Data model

IPLD throughout. DAG-CBOR encoded. CIDs use BLAKE3. Every `Signed<T>` envelope carries `version: u8 = 1` as the first field for forward compatibility.

### Causal vs provenance ancestry

`Signed<T>` separates two ancestry relations:

- **`causal: Vec<Cid>`** — CRDT predecessors (empty for non-Op envelopes).
- **`provenance: Vec<Cid>`** — semantic ancestry (sources, derivations).

### Visibility

A first-class envelope field: `Internal | Federated | Public`. Enforced at gossip-discovery and as defense in depth at block-serve.

### Classification

A tag-scope convention (`classification:<level>`) enforced by the linter at write time. Default levels: `public | internal | confidential`. Composes with `Grant` patterns directly.

### Signing

Every logical write produces a `Signed<T>`. Signing covers everything except the `signature` field, over canonical DAG-CBOR.

### Block exchange

`beetswap::Behaviour`, allowed only on authenticated connections. Per-connection visibility check at serve time.

### Convergence

Multi-writer CRDT via Loro. Writes are operations published as `Signed<Op>` IPLD blocks. Sync is "fetch op-blocks I haven't seen, merge in causal order." Snapshots published periodically.

### Snapshot and op trust

Ops are authoritative; snapshots are convenience. `Snapshot::cluster_only = true` by default. Cross-cluster snapshot sharing requires dual-admin approval (Phase 8).

### Garbage collection

**Manual.** Operators run `memvault gc --doc <id> --before <time>`. Cooperative GC is Phase 8.

### Tag discipline

`scope:label` enforced by the linter. Required: exactly one `classification:<level>`. Optional: `agent:`, `kind:`, `topic:`, `session:`, `provenance:`, `handling:`, `compartment:`.

### History, audit, retraction

History via CRDT log replay. Audit via envelope-metadata projection. Retraction via signed tombstones.

### Quotas and backpressure

Per-agent rate limits and storage caps in Phase 3. In-memory only for v1.

### Summarization

A built-in service. Summaries are `Signed<Summary>` envelopes with `provenance = sources`. The LLM client is abstract.

### Web interface

Dioxus webapp with structural-only components. Documented `mv-*` class names in `STYLES.md`. Consumers replace `default.css`.

---

## 4. Threat model

### What we trust

- **Cluster admin keys** (catastrophic if compromised; multi-sig is Phase 8).
- **Authenticated intra-cluster peers** with all blocks at the bitswap layer.
- **Authenticated federation peers** with `Public` blocks unconditionally and `Federated` blocks where grants apply.
- **Agent keys** scoped to that agent's capabilities.

### What we don't trust

- Unauthenticated network actors (cannot complete handshake).
- Wall clocks (Lamport ordering authoritative).
- Self-attribution (only signatures are authoritative).
- Snapshots from other clusters.
- External destinations.

### Why no transport PSK

PSK is binary. The handshake gives us per-peer revocation, role distinction, TTL.

### Why bitswap is not authenticated per-block

Protocol limitation. Discovery-as-access-control + serve-time visibility check.

### Token threat model

- **Tokens are bearer credentials** during their validity window. Anyone with a valid unconsumed token gets the attached role at redemption time. Treat tokens as secret in transit (out-of-band sharing should use secure channels).
- **Tokens are short-lived and single-use** by default. Long-lived or multi-use tokens are an explicit choice (admin sets `not_after_ns` and `max_uses`).
- **Consumed-token tracking is gossipsub-best-effort.** A token presented to two admins simultaneously could in principle redeem twice if `max_uses=1` but the consumption announcements race. Mitigation: redeeming admin checks the `consumed_tokens` table immediately before issuing the attestation; if a duplicate is detected post-hoc via gossip, the later attestation is treated as suspect and revoked. For higher-stakes deployments, restrict token redemption to a single primary admin (Phase 8 multi-admin coordination).

### Rotation threat model

- **Both old and new key sign the rotation block.** Old proves authorization; new proves possession (so an attacker who obtains an old admin key cannot rotate to a key they don't control — the new key must sign too).
- **Overlap window covers in-flight signatures.** During the window, both keys are accepted. Outside the window, only the new key. If a key is compromised mid-rotation, the rotation can be aborted before `valid_from_ns` by an admin signing a `RotationAborted` block.
- **Cluster identity is stable across rotation.** The ClusterId is independent of any key, so federation peers don't need to update their notion of "which cluster is this?" — only the admin key list under that ClusterId.

---

## 5. Data shapes

All structures are DAG-CBOR encoded. `Signed<T>` carries `version: u8 = 1` as the first field.

### 5.1 Signed envelope

```rust
struct Signed<T> {
    version:    u8,                 // 1
    payload:    T,
    author:     PeerId,
    causal:     Vec<Cid>,
    provenance: Vec<Cid>,
    tags:       Vec<Tag>,
    visibility: Visibility,
    lamport:    u64,
    wall_ns:    u64,
    capability: Option<Cid>,
    signature:  [u8; 64],
}
```

### 5.2 Tags and visibility

```rust
struct Tag { scope: String, label: String }
struct TagPattern { scope: String, label: GlobPattern }
enum Visibility { Internal, Federated, Public }
```

### 5.3 Classification (tag conventions)

Required: exactly one `classification:<level>` tag from {`public`, `internal`, `confidential`}. Optional: `handling:<flag>` (`pii`, `phi`), `compartment:<name>`.

### 5.4 Markdown documents

```rust
struct Document {
    id:          DocId,
    body:        LoroText,
    frontmatter: LoroMap,
}
```

### 5.5 Knowledge graphs

```rust
struct Entity {
    id:        EntityId,
    kind:      String,
    props:     LoroMap,
    edges_out: LoroMovableList<Edge>,
}

struct Edge {
    id:         EdgeId,
    relation:   String,
    target:     EntityId,           // stable ID, NOT a CID
    weight:     Option<f32>,
    props:      LoroMap,
    provenance: Option<Cid>,
}
```

### 5.6 Authentication structures

```rust
struct ClusterId([u8; 32]);         // stable random; independent of admin keys

struct MembershipAttestation {
    cluster_id:   ClusterId,
    member:       PeerId,           // INVARIANT: == connecting peer's id
    role:         Role,
    not_after_ns: u64,
    issued_via:   AttestationOrigin, // Direct | TokenRedemption(Cid)
    signature:    [u8; 64],         // by an admin key valid at not_before time
}

enum AttestationOrigin { Direct, TokenRedemption(Cid) }

enum Role { Admin, AgentHost, Auditor, Service }

struct Grant {
    issuer:          PeerId,
    issuing_cluster: ClusterId,
    audience:        GrantAudience,
    scopes:          Vec<TagPattern>,
    actions:         Vec<Action>,
    not_before_ns:   u64,
    not_after_ns:    u64,
    parent:          Option<Cid>,
    nonce:           [u8; 16],
    signature:       [u8; 64],
}

enum GrantAudience { Cluster(ClusterId), Peer(PeerId), Agent(AgentId), Role(Role) }
enum Action { Read, Write, Admin, Egress }

struct Revocation {
    target:        Cid,
    reason:        String,
    revoked_at_ns: u64,
    signature:     [u8; 64],
}
```

### 5.7 Agent identity

```rust
struct AgentId(String);

struct AgentEnrollment {
    agent_id:       AgentId,
    public_key:     PublicKey,
    cluster_id:     ClusterId,
    enrolled_by:    PeerId,
    initial_grants: Vec<Cid>,
    not_after_ns:   u64,
    signature:      [u8; 64],
}
```

### 5.8 Join tokens

```rust
struct JoinToken {
    issuer:         PeerId,         // admin who minted the token
    cluster_id:     ClusterId,
    role:           Role,           // role granted to redeemer
    initial_grants: Vec<Cid>,       // optional capabilities for redeemer
    not_before_ns:  u64,
    not_after_ns:   u64,
    max_uses:       u32,            // typically 1
    nonce:          [u8; 16],       // randomness
    label:          Option<String>, // human-readable label for UX (e.g., "openclaw-host-3")
    signature:      [u8; 64],
}

struct TokenConsumption {
    token_cid:      Cid,
    consumer:       PeerId,         // PeerId of the redeeming peer
    consumed_at_ns: u64,
    issued_attestation: Cid,        // CID of the resulting MembershipAttestation
    signature:      [u8; 64],       // by the redeeming admin
}
```

A `Signed<JoinToken>` is shared out-of-band. The token's wire format for sharing (URL, CLI paste, QR code) is a base32-encoded CID-prefixed string:

```
mvjoin1:<base32(cbor(Signed<JoinToken>))>
```

The `mvjoin1:` prefix identifies the format version. The full token block is embedded so the redeeming peer can verify without first fetching anything.

### 5.9 Key rotation

```rust
struct AdminKeyRotation {
    cluster_id:        ClusterId,
    old_key:           PublicKey,
    new_key:           PublicKey,
    valid_from_ns:     u64,
    overlap_until_ns:  u64,         // both keys valid until this time
    rotation_id:       [u8; 16],    // randomness; used by RotationAborted
    signature_old:     [u8; 64],    // old key signs the (cluster_id, new_key, timestamps, rotation_id) tuple
    signature_new:     [u8; 64],    // new key signs the same tuple (proof of possession)
}

struct AgentKeyRotation {
    agent_id:          AgentId,
    cluster_id:        ClusterId,
    old_key:           PublicKey,
    new_key:           PublicKey,
    valid_from_ns:     u64,
    overlap_until_ns:  u64,
    rotation_id:       [u8; 16],
    signature_old:     [u8; 64],
    signature_new:     [u8; 64],
}

struct RotationAborted {
    rotation_id:       [u8; 16],
    aborted_at_ns:     u64,
    reason:            String,
    signature:         [u8; 64],    // by an admin (any current admin can abort an admin rotation)
}
```

Rotation blocks are wrapped in `Signed<T>` (signed by the rotating party's libp2p key, separate concern from `signature_old`/`signature_new` which are the rotation-specific dual signature).

### 5.10 Federation trust

```rust
struct ClusterTrust {
    cluster_id:         ClusterId,
    trusted_admin_keys: Vec<PublicKey>,    // updated as the other cluster rotates
    federated_since_ns: u64,
    not_after_ns:       u64,
    signature:          [u8; 64],
}
```

### 5.11 CRDT operations and snapshots

```rust
struct Op { doc_id: DocId, lamport: u64, body: LoroOp }

struct Snapshot {
    doc_id:        DocId,
    covers_ops:    Vec<Cid>,
    state:         Vec<u8>,
    state_version: u8,
    cluster_only:  bool,            // default true
}
```

### 5.12 Document heads

```rust
struct DocumentHead {
    doc_id:       DocId,
    writer:       PeerId,
    snapshot_cid: Option<Cid>,
    frontier:     Vec<Cid>,
    epoch:        u64,
    visibility:   Visibility,
}
```

### 5.13 Audit records (a view, not a stored entity)

```rust
struct AuditRecord {
    envelope:    Cid,
    author:      PeerId,
    wall_ns:     u64,
    tags:        Vec<Tag>,
    visibility:  Visibility,
    causal:      Vec<Cid>,
    provenance:  Vec<Cid>,
    op_kind:     OpKind,
    capability:  Option<Cid>,
    affected:    Affected,
}

enum OpKind {
    Created, Updated, Snapshotted, Retracted,
    EgressChecked, EgressPerformed,
    TokenIssued, TokenRedeemed,
    AdminKeyRotated, AgentKeyRotated, RotationAborted,
    AttestationRenewed,
}

struct AuditQuery { /* author, time, tag_pattern, op_kind, etc. */ }
```

### 5.14 Summary memories

```rust
trait LlmClient: Send + Sync {
    async fn summarize(&self, prompt: String, ctx: Vec<String>) -> Result<String>;
}

struct SummarizationRequest { /* scope, kind, horizon, max_input_tokens */ }

struct Summary {
    request:         SummarizationRequest,
    sources:         Vec<Cid>,
    output:          String,
    model_id:        String,
    generated_at_ns: u64,
}
```

### 5.15 PII findings and redactions

```rust
struct PiiFinding { kind: PiiKind, location: Location, text: String, confidence: f32, detector: String }
enum PiiKind { Email, Phone, Ssn, CreditCard, PersonName, StreetAddress, IpAddress, Iban, DateOfBirth, HealthRecordNumber, Custom(String) }
struct RedactionPolicy { by_kind: BTreeMap<PiiKind, RedactionStrategy>, default: RedactionStrategy }
enum RedactionStrategy { Mask, Drop, Hash, Tokenize, Replace(String) }
struct RedactionResult { output_cid: Cid, report_cid: Cid, findings: Vec<PiiFinding>, applied: Vec<RedactionApplied> }
```

### 5.16 Egress

```rust
struct EgressDestination { name: String, kind: EgressKind, url: Option<String> }
enum EgressKind { CloudLlm, Backup, AgentHost, ThirdParty, PublicShare }
enum EgressDecision {
    Allow,
    Deny { reasons: Vec<String> },
    AllowWithRedaction { suggested_policy: RedactionPolicy, blocking: Vec<PiiFinding> },
}
```

---

## 6. Layered architecture

```
┌──────────────────────────────────────────────────────────────┐
│ L9  web UI (Dioxus, structural only)                         │  Phase 7
├──────────────────────────────────────────────────────────────┤
│ L8  agent API + HTTP/JSON-RPC bridge + memctl                │  Phase 3 / 7
├──────────────────────────────────────────────────────────────┤
│ L7  classification, PII, summarization                       │  Phase 4 / 5
├──────────────────────────────────────────────────────────────┤
│ L6  history / audit / retraction (memvault-query)            │  Phase 2/3
├──────────────────────────────────────────────────────────────┤
│ L5  index + quotas (memvault-query)                          │  Phase 3
├──────────────────────────────────────────────────────────────┤
│ L4  CRDT engine + materialized view                          │  Phase 2
├──────────────────────────────────────────────────────────────┤
│ L3  signed-envelope + auth/cap/tokens/rotation               │  Phase 1
├──────────────────────────────────────────────────────────────┤
│ L2  blockstore                                               │  Phase 1
├──────────────────────────────────────────────────────────────┤
│ L1  network: Swarm + auth + join + bitswap + gossip + kad    │  Phase 1, 6
├──────────────────────────────────────────────────────────────┤
│ L0  libp2p (existing)                                        │  done
└──────────────────────────────────────────────────────────────┘
```

---

## 7. Federation

Federation reuses intra-cluster primitives:

1. **Trust setup.** `ClusterTrust` set lists trusted clusters. The verifier accepts attestations signed by any of their **currently valid** admin keys (rotation-aware).
2. **Per-connection classification.** Auth handshake records each connection's `cluster_id`.
3. **Visibility enforcement.** `Internal` only intra; `Federated` requires grant; `Public` any auth'd connection.
4. **Discovery boundary.** Gossip filters head announcements per-recipient.
5. **Cross-cluster grants.** Issued on the federation topic; cached locally.
6. **Classification at federation egress.** Grants reference `classification:*` tag patterns.
7. **Cross-cluster rotation propagation.** When cluster B rotates an admin key, the rotation block is gossiped on the federation topic; A receives it and updates its `ClusterTrust.trusted_admin_keys` list.

```rust
enum FederationAnnouncement {
    HeadAvailable { head_cid: Cid, visibility: Visibility, scope: Vec<Tag> },
    GrantIssued { grant_cid: Cid },
    GrantRevoked { revocation_cid: Cid },
    TrustRevoked { revocation_cid: Cid },
    AdminKeyRotated { rotation_cid: Cid },
}
```

### Why one swarm

Auth state is per-connection regardless. Visibility enforcement is per-connection regardless. A single swarm halves operational surface.

---

## 8. Classification, PII, and egress control

### Classification as tag convention

Lives in tags. Composes with grants. Composes with indexes. Extensible via Phase 8 config.

### PII detection

`PiiDetector` trait with default regex + Aho-Corasick impl. Runs at write time (strict) and on demand.

### PII cleaner

`PiiCleaner` trait. Produces a redacted `Signed<Document>` (with `kind:redacted`, `provenance: [source]`) plus a `Signed<RedactedDocument>` audit report.

### Egress policy

Hard-coded default policies for v1; TOML loader Phase 8. Two-layer control: `Action::Egress` capability AND per-destination policy.

### Audit integration

`check_egress` and successful egress operations generate audit records.

### LLM redaction is itself egress

Egress check runs *before* the LLM call.

---

## 9. Token-based admission

### Why tokens

Manually crafting attestation files for every new peer is operationally awful. Tokens let an admin generate a short string that a new peer can use to enroll itself, similar to Kubernetes bootstrap tokens or Tailscale auth keys. The pattern is well-understood and battle-tested.

### Token lifecycle

```
admin           seeder/admin peer            new peer
  │                    │                        │
  │ 1. mint Signed<JoinToken>                   │
  │  ─────────────────────────► out-of-band ──► │
  │                    │                        │
  │                    │   2. /ai-memvault/join/1.0 (with token)
  │                    │ ◄──────────────────────┤
  │                    │                        │
  │                    │   3. verify token,     │
  │                    │      check consumed_tokens,
  │                    │      sign attestation, │
  │                    │      gossip TokenConsumption
  │                    │                        │
  │                    │   4. return attestation│
  │                    │ ──────────────────────►│
  │                    │                        │
  │                    │                        │ 5. store attestation,
  │                    │                        │    use for /auth/1.0
```

### Generating a token

An admin runs (in CLI or via API):

```bash
memctl token issue --role agent-host --ttl 1h --max-uses 1 --label "openclaw-host-3"
```

Output is a `mvjoin1:...` string. The admin shares it out-of-band (chat, ticket, env var, config-management push).

### Redeeming a token

A new peer runs:

```bash
memvault join --token mvjoin1:...
```

The new peer:
1. Generates a fresh libp2p Ed25519 keypair if none exists at `${MEMVAULT_DATA_DIR}/identity/peer.key`.
2. Reads bootstrap peer addresses from config or arguments.
3. Connects to a bootstrap peer, opens a `/ai-memvault/join/1.0` stream.
4. Sends a `JoinRequest` containing the token bytes and its own PeerId.
5. Receives a `JoinResponse` with the `Signed<MembershipAttestation>`.
6. Writes the attestation to `${MEMVAULT_DATA_DIR}/identity/attestation.cbor`.
7. Disconnects and reconnects normally via `/ai-memvault/auth/1.0`.

### Server-side handling

The receiving peer (must hold `Action::Admin` capability):

1. Decodes the token; verifies its signature against currently valid admin keys.
2. Checks `not_before_ns ≤ now ≤ not_after_ns`.
3. Checks `consumed_tokens` table for prior redemption count; if `consumed_count >= max_uses`, deny.
4. Verifies the requested PeerId matches the connecting peer.
5. Constructs a `Signed<MembershipAttestation>` for the joining PeerId, with `issued_via: TokenRedemption(token_cid)`, `role` from the token, TTL from a configurable default (24h) but capped by token's `not_after_ns`.
6. Writes a `Signed<TokenConsumption>` block recording the redemption.
7. Gossips both the attestation and the consumption on `ai-memvault/admin/v1`.
8. Returns the attestation in the `JoinResponse`.

If the receiving peer does not hold `Action::Admin`, it rejects the request with `JoinError::NotAdminPeer { try_peers: [...] }`. The joiner retries on a different bootstrap peer.

### Race conditions

If two peers attempt to redeem the same `max_uses=1` token nearly simultaneously against different admin peers, both may succeed locally before consumption gossip reconciles. Handling:

- Once gossip reconciles, the redeeming admins observe the duplicate. The earlier `TokenConsumption` (by `wall_ns` then by tiebreak on `consumer` PeerId) is canonical; the later attestation is invalidated by issuing a `Revocation` for it.
- The newly-attested peer that ends up with a revoked attestation discovers this on its next auth handshake and re-runs `memvault join` with a fresh token.

This is acceptable for the vast majority of cases. For high-stakes deployments, restrict token redemption to a single primary admin (Phase 8 multi-admin coordination).

### Token revocation

A `Revocation` block targeting a token CID prevents future redemption. Existing attestations issued via the token are unaffected (revoke them separately if desired).

---

## 10. Key rotation

### What rotates

| Key | Owner | Rotation cadence | Mechanism |
|---|---|---|---|
| Admin key | cluster | periodic (90d default) | `AdminKeyRotation` block |
| Agent key | agent | periodic (180d default) | `AgentKeyRotation` block |
| Peer libp2p key | peer | rare; treated as re-enrollment | new `JoinToken` for new PeerId |
| Membership attestation | peer | continuous (TTL renewal) | new attestation issued by admin |

### Admin key rotation

The rotating admin generates a new keypair, then constructs a `Signed<AdminKeyRotation>` block:

- `signature_old` is the old key signing the rotation tuple.
- `signature_new` is the new key signing the same tuple. Required to prove possession — without it, an attacker who steals the old key could rotate to a key they don't control.

The block is written via the normal write path and gossiped on `ai-memvault/admin/v1`. All peers update their local admin-key view:

- During `[valid_from_ns, overlap_until_ns]`, both old and new key are accepted by `AuthVerifier`.
- After `overlap_until_ns`, only the new key is accepted; the old key is removed from the active set.

Federated peers receive the rotation via federation gossip and update their `ClusterTrust.trusted_admin_keys`.

### Aborting a rotation

If an in-flight rotation needs to be canceled (compromised key, mistake, etc.) before `valid_from_ns`, any current admin signs a `Signed<RotationAborted>` block referencing the `rotation_id`. Verifiers see this and ignore the rotation block.

### Agent key rotation

Same shape as admin rotation, but agent-scoped. The agent generates a new keypair and constructs `Signed<AgentKeyRotation>` signed by both old and new agent key. The agent's local `enrollment.cbor` is updated.

Important: existing envelopes signed by the agent's old key remain valid during and after the overlap. Verifiers consult the `AgentKeyRotation` block to know when to start accepting the new key.

### Peer libp2p key rotation

Treated as re-enrollment. Generate a new key, request a new `JoinToken` from an admin, run `memvault join`. The old peer's `MembershipAttestation` is revoked. Why: libp2p keys are conventionally long-lived, and "rotating" a libp2p key is effectively replacing the peer.

### Attestation renewal

Attestations have a TTL (default 24h). Before expiry, the peer requests a renewal via `memctl renew-attestation` (or via the API). An admin signs a fresh attestation for the same `(cluster_id, member, role)` with extended TTL. The peer replaces its local attestation file.

This is a routine operation, automatable via cron or systemd timer. The renewing admin must hold `Action::Admin` capability.

### Verifier rotation awareness

`AuthVerifier` maintains:

```rust
struct AdminKeyState {
    keys: BTreeMap<PublicKey, KeyValidity>,
}

struct KeyValidity {
    valid_from_ns:  u64,
    valid_until_ns: u64,                // u64::MAX if no rotation has retired this key yet
    introduced_by:  Option<Cid>,        // CID of AdminKeyRotation that introduced it (None for genesis)
}
```

The verifier's `verify_attestation` walks: "was this attestation signed by a key valid at the attestation's signing time?" Signing time is `attestation.not_after_ns - default_ttl`, give or take — we use the rotation's overlap window as the source of truth.

For envelopes: `verify_envelope` uses `wall_ns` as the timestamp for "what keys were valid then." Agent key rotation uses the same logic for agent-signed envelopes.

---

## 11. Web interface

A Dioxus webapp + axum server, packaged as one crate (`memvault-web`) with two binaries: `memvault-web-server` (axum) and `memvault-web-app` (Dioxus WASM). Structural only.

### Phase 7 components (5)

| Component | Class prefix | Renders |
|---|---|---|
| `MemoryList` | `mv-memory-list` | filterable list of memories |
| `MemoryDetail` | `mv-memory-detail` | markdown + frontmatter + history + classification badges |
| `AuditLog` | `mv-audit-log` | filtered audit records |
| `EgressGate` | `mv-egress-gate` | run check_egress, show decision |
| `AdminPanel` | `mv-admin-panel` | peers, grants, trust, revocations, **tokens**, **rotation status** |

The `AdminPanel` Phase 7 surface includes:
- Token issuance form (role, TTL, max-uses, label)
- Token list (active, consumed, revoked) with revoke action
- Admin key rotation history + abort-pending-rotation button
- Per-agent rotation history
- Pending-attestation-renewal queue

Phase 8 adds: `PiiReview`, `EntityBrowser`, `SummaryRequest`, `RetractionForm`, `ClassificationPanel`. Their CLI equivalents in `memctl` cover Phase 7 operationally.

### Style philosophy

Semantic HTML only. Documented `mv-*` class names in `STYLES.md`. No inline styles, no Tailwind. Replace `default.css` to theme.

### Authentication

Webapp authenticates as an agent via signed bearer tokens (4h TTL).

---

## 12. Workspace structure

```
memvault/
├── Cargo.toml
├── crates/
│   ├── memvault-core/               # data types
│   ├── memvault-auth/               # attestations, grants, tokens, rotation
│   ├── memvault-store/              # blockstore + indexes
│   ├── memvault-net/                # libp2p behaviours, auth + join handshakes
│   ├── memvault-doc/                # CRDT documents + graphs
│   ├── memvault-query/              # history + audit + index + retraction
│   ├── memvault-policy/             # classification + PII + egress
│   ├── memvault-summarize/          # summarization service
│   ├── memvault-api/                # MemvaultClient + RPC + memctl bin
│   └── memvault-web/                # axum server + Dioxus WASM
├── tests/integration/
└── benches/
```

### `memvault-core` (Phase 1)

```
src/
├── lib.rs
├── cid.rs
├── envelope.rs         # Signed<T>
├── codec.rs
├── version.rs
├── tags.rs
├── tags_lint.rs        # incl. classification rules
├── classification.rs
├── visibility.rs
├── time.rs
├── ids.rs
└── error.rs
```

### `memvault-auth` (Phase 1)

```
src/
├── lib.rs
├── attestation.rs      # MembershipAttestation + verify (member==peer; rotation-aware)
├── grant.rs
├── revocation.rs
├── role.rs
├── enrollment.rs
├── trust.rs
├── token.rs            # JoinToken + TokenConsumption + token-string codec
├── rotation.rs         # AdminKeyRotation + AgentKeyRotation + RotationAborted
├── key_state.rs        # AdminKeyState evolution; "valid at time T" queries
├── verifier.rs         # AuthVerifier (rotation-aware; intra + federation)
└── error.rs
```

### `memvault-store` (Phase 1)

```
src/
├── lib.rs
├── blockstore.rs
├── tables.rs
├── insert.rs
├── query.rs
├── audit_index.rs
├── retracted.rs
├── consumed_tokens.rs  # TokenConsumption tracking
├── rotation_state.rs   # current admin key state; reconstructed on startup
└── error.rs
```

redb tables (13):

| Table | Key | Value |
|---|---|---|
| `blocks` | `Cid` | block bytes |
| `by_tag` | `(Tag, wall_ns, Cid)` | `()` |
| `by_author` | `(PeerId, wall_ns, Cid)` | `()` |
| `by_time` | `(wall_ns, Cid)` | `()` |
| `by_causal` | `(parent_cid, child_cid)` | `()` |
| `by_provenance` | `(parent_cid, child_cid)` | `()` |
| `edges` | `(EdgeKind, EntityId, EntityId, wall_ns)` | `Cid` |
| `heads` | `(DocId, PeerId)` | `Cid` |
| `revocations` | `Cid` | `Revocation` |
| `retracted` | `Cid` | `Cid` |
| `cluster_origin` | `(ClusterId, wall_ns, Cid)` | `()` |
| `consumed_tokens` | `Cid` (token CID) | `(consumed_count, last_consumer, last_consumed_at)` |
| `rotations` | `(rotation_id, wall_ns)` | `Cid` |

### `memvault-net` (Phase 1, federation in Phase 6)

```
src/
├── lib.rs
├── swarm.rs
├── behaviour.rs
├── auth_proto/
│   ├── mod.rs
│   ├── codec.rs
│   ├── server.rs
│   └── client.rs
├── join_proto/             # /ai-memvault/join/1.0
│   ├── mod.rs
│   ├── codec.rs
│   ├── server.rs           # admin-side: verify token, issue attestation
│   └── client.rs           # joiner-side: present token, receive attestation
├── gater.rs
├── conn_state.rs
├── bitswap.rs
├── gossip.rs               # heads, admin (rotation+consumption), federation
├── federation.rs           # Phase 6
└── discovery.rs
```

### `memvault-doc` (Phase 2)

```
src/
├── lib.rs
├── document.rs
├── graph.rs
├── op.rs
├── log.rs
├── snapshot.rs
├── compaction.rs
├── head.rs
└── apply.rs
```

### `memvault-query` (Phase 2/3)

```
src/
├── lib.rs
├── history/
│   ├── time_travel.rs
│   ├── checkpoint.rs
│   ├── trace.rs
│   └── diff.rs
├── audit/
│   ├── query.rs
│   └── retraction.rs
├── index/
│   ├── schema.rs
│   ├── ingest.rs
│   ├── rebuild.rs
│   ├── search.rs
│   └── effective_tags.rs
└── error.rs
```

### `memvault-policy` (Phase 5)

```
src/
├── lib.rs
├── pii/                    # detector trait + default regex impl
├── cleaner/                # redaction strategies + report
├── egress/                 # policy + decision + audit log
└── error.rs
```

### `memvault-summarize` (Phase 4)

```
src/
├── lib.rs
├── service.rs
├── llm.rs
├── prompts.rs
├── scope.rs
├── output.rs
├── cache.rs
└── invalidate.rs
```

### `memvault-api` (Phase 3)

```
src/
├── lib.rs
├── client.rs               # MemvaultClient trait
├── local.rs
├── rpc/
│   ├── mod.rs
│   ├── server.rs
│   ├── client.rs
│   └── auth.rs
├── tokens.rs               # token issuance + redemption helpers
├── rotation.rs             # rotation operation helpers
├── quotas.rs
├── subscription.rs
├── types.rs
└── bin/
    └── memctl.rs
```

```rust
trait MemvaultClient {
    // memories
    async fn put_doc(&self, doc: Document, tags: Vec<Tag>, vis: Visibility) -> Result<Cid>;
    async fn get_doc(&self, id: DocId) -> Result<Document>;
    async fn query(&self, q: Query) -> Result<Vec<Hit>>;
    async fn subscribe(&self, scope: TagPattern) -> Result<EventStream>;

    // graph
    async fn add_entity(&self, e: Entity, vis: Visibility) -> Result<EntityId>;
    async fn add_edge(&self, edge: Edge, vis: Visibility) -> Result<EdgeId>;
    async fn traverse(&self, from: EntityId, spec: TraversalSpec) -> Result<Vec<EntityId>>;

    // lifecycle
    async fn retract(&self, target: Cid, reason: RetractionReason, legal_hold: bool) -> Result<Cid>;
    async fn history_of(&self, target: HistoryTarget, at: SystemTime) -> Result<HistoricView>;
    async fn audit(&self, q: AuditQuery) -> Result<Vec<AuditRecord>>;

    // policy
    async fn detect_pii(&self, target: Cid) -> Result<Vec<PiiFinding>>;
    async fn check_egress(&self, target: Cid, dest: EgressDestination) -> Result<EgressDecision>;
    async fn redact(&self, target: Cid, policy: RedactionPolicy) -> Result<RedactionResult>;

    // summarize
    async fn summarize_scope(&self, req: SummarizationRequest) -> Result<Cid>;

    // ops
    async fn gc(&self, doc: DocId, before: SystemTime) -> Result<GcReport>;

    // tokens
    async fn issue_token(&self, role: Role, ttl: Duration, max_uses: u32, label: Option<String>)
        -> Result<JoinTokenString>;
    async fn redeem_token(&self, token: JoinTokenString, my_peer_id: PeerId)
        -> Result<MembershipAttestation>;
    async fn revoke_token(&self, token_cid: Cid, reason: String) -> Result<Cid>;
    async fn list_tokens(&self) -> Result<Vec<TokenStatus>>;

    // rotation
    async fn rotate_admin_key(&self, new_key: PublicKey, overlap: Duration) -> Result<Cid>;
    async fn rotate_agent_key(&self, agent_id: AgentId, new_key: PublicKey, overlap: Duration)
        -> Result<Cid>;
    async fn abort_rotation(&self, rotation_id: [u8; 16], reason: String) -> Result<Cid>;
    async fn renew_attestation(&self, target: PeerId) -> Result<MembershipAttestation>;
    async fn list_rotations(&self) -> Result<Vec<RotationStatus>>;
}
```

### `memvault-web` (Phase 7)

```
crates/memvault-web/
├── Cargo.toml
├── STYLES.md
├── src/
│   ├── lib.rs
│   ├── server/             # axum binary
│   │   ├── main.rs
│   │   ├── auth.rs
│   │   ├── rpc.rs
│   │   ├── subscribe.rs
│   │   └── static_assets.rs
│   ├── app/                # WASM binary
│   │   ├── main.rs
│   │   ├── api_client.rs
│   │   ├── auth.rs
│   │   ├── routes.rs
│   │   └── components/
│   │       ├── memory_list.rs
│   │       ├── memory_detail.rs
│   │       ├── audit_log.rs
│   │       ├── egress_gate.rs
│   │       └── admin_panel.rs    # incl. tokens + rotation
└── assets/
    ├── default.css
    └── index.html
```

---

## 13. Cargo dependencies

```toml
ipld-core            = "*"
serde_ipld_dagcbor   = "*"
cid                  = "*"
multihash-codetable  = { version = "*", features = ["blake3", "sha2"] }
serde                = { version = "*", features = ["derive"] }

libp2p               = { version = "*", features = [
    "tcp", "quic", "noise", "yamux",
    "gossipsub", "kad", "identify", "tls",
    "request-response", "macros"
] }
beetswap             = "*"
blockstore           = "*"

redb                 = "*"
tantivy              = "*"           # Phase 3

loro                 = "*"

tarpc                = { version = "*", features = ["tokio1"] }
tokio                = { version = "*", features = ["full"] }
axum                 = "*"           # Phase 7
tower-http           = "*"           # Phase 7

dioxus               = { version = "*", features = ["web"] }   # Phase 7
dioxus-web           = "*"           # Phase 7

regex                = "*"           # Phase 5
aho-corasick         = "*"           # Phase 5

ed25519-dalek        = "*"
blake3               = "*"
thiserror            = "*"
tracing              = "*"
governor             = "*"           # rate limiting (Phase 3)
pulldown-cmark       = "*"           # markdown rendering in webapp
data-encoding        = "*"           # base32 token encoding (Phase 1)
```

---

## 14. Wire protocols

### 14.1 `/ai-memvault/auth/1.0` — connection auth handshake

Initiator sends `AuthRequest`; responder verifies and replies with `AuthResponse`. Both contain `attestation_block`, `attestation` CID, `cluster_id`, `version`. Receiver checks attestation parses, signature verifies under a currently-valid admin key, `attestation.cluster_id` matches, `attestation.member == sender's PeerId` (verified by Noise/TLS), not expired, not revoked.

No proof-of-possession nonce. Libp2p's transport handshake authenticates the PeerId.

### 14.2 `/ai-memvault/join/1.0` — token redemption (Phase 1)

```
JoinRequest {
    version:       u8,                  // = 1
    token_block:   Bytes,               // serialized Signed<JoinToken>
    peer_id:       PeerId,              // joiner's PeerId
    requested_ttl: Option<u64>,         // proposed attestation TTL in seconds
}

JoinResponse {
    version:           u8,
    result:            JoinResult,
}

enum JoinResult {
    Success { attestation_block: Bytes },
    Refuse  { reason: JoinRefuseReason, try_peers: Vec<PeerAddress> },
}

enum JoinRefuseReason {
    NotAdminPeer,
    TokenInvalidSignature,
    TokenExpired,
    TokenNotYetValid,
    TokenAlreadyConsumed,
    TokenRevoked,
    PeerIdMismatch,
    RoleNotAllowed,
    RateLimited,
}
```

The receiving peer verifies the token, checks the consumed_tokens table, signs a `Signed<MembershipAttestation>`, gossips a `Signed<TokenConsumption>` block on the admin topic, and returns the attestation. If the receiving peer does not hold `Action::Admin`, it returns `NotAdminPeer` with a list of known admin peer addresses for the joiner to try next.

This protocol is reachable on connections that have NOT yet completed the auth handshake (since the joining peer doesn't have an attestation yet). It is the only protocol allowed on un-authed connections; bitswap and gossipsub remain gated.

### 14.3 Block exchange via beetswap

Standard bitswap on authenticated connections.

### 14.4 Heads gossip — `ai-memvault/heads/v1`

Per-cluster gossipsub.

### 14.5 Admin gossip — `ai-memvault/admin/v1` (Phase 1)

Carries:
- `TokenConsumption` announcements (so other admins update their `consumed_tokens` table).
- `AdminKeyRotation` announcements (so all peers update their admin key state).
- `RotationAborted` announcements.
- `Revocation` announcements for attestations, grants, and tokens.

```rust
enum AdminAnnouncement {
    TokenConsumed(Cid),
    AdminKeyRotated(Cid),
    AgentKeyRotated(Cid),
    RotationAborted(Cid),
    Revoked(Cid),
}
```

### 14.6 Federation gossip — `ai-memvault/federation/v1` (Phase 6)

Per-federation-pair topic; carries `FederationAnnouncement`s including admin rotation propagation.

### 14.7 HTTP API — `memvault-web-server` (Phase 7)

```
POST   /api/v1/auth           — issue bearer token (signed challenge)
POST   /api/v1/rpc            — JSON-RPC 2.0; mirrors MemvaultClient
GET    /api/v1/subscribe      — SSE stream
GET    /static/*              — WASM bundle + assets
GET    /                      — index.html
```

---

## 15. Filesystem layout

```
${MEMVAULT_DATA_DIR}/
├── config.toml
├── identity/
│   ├── peer.key                   # libp2p Ed25519; 0600
│   ├── peer.pub
│   └── attestation.cbor           # current MembershipAttestation
├── trust/
│   ├── admins.cbor                # current admin key set + rotation history
│   ├── clusters.cbor              # ClusterTrust entries
│   └── revocations.cbor
├── agents/
│   └── <agent_id>/
│       ├── identity/
│       │   ├── agent.key
│       │   ├── agent.pub
│       │   ├── enrollment.cbor
│       │   └── rotations.cbor     # AgentKeyRotation history
│       └── grants.cbor
├── blocks.redb
├── tantivy/
├── snapshots/
├── logs/
└── tmp/
```

`config.toml` adds:

```toml
[admission]
default_token_ttl_minutes = 60
default_token_max_uses = 1
default_attestation_ttl_hours = 24
allow_renewal = true

[rotation]
default_admin_overlap_days = 7
default_agent_overlap_days = 14
```

---

## 16. Phase plan

### Phase 1 — Authenticated storage + sync + token join + basic rotation ★ milestone

**Goal:** Two cluster peers complete the auth handshake, exchange signed envelopes, and the audit index is populated. New peers join via tokens. Admin key rotation primitives exist and are exercised.

**Deliverables**

1. `memvault-core`: full data shapes incl. version byte, split causal/provenance, classification linter rules.
2. `memvault-auth`: `MembershipAttestation` (member==peer invariant), `Grant`, `Revocation`, `AgentEnrollment`, `ClusterTrust`, `JoinToken`, `TokenConsumption`, `AdminKeyRotation`, `AgentKeyRotation`, `RotationAborted`. `AuthVerifier` with rotation-aware "valid at time T" queries. Token-string codec (`mvjoin1:` prefix, base32 body).
3. `memvault-store`: redb-backed blockstore, 13 index tables, atomic insert, audit projection, `consumed_tokens` table, rotation state reconstruction on startup.
4. `memvault-net`:
   - `ClusterSwarm` builder (no PSK).
   - `/ai-memvault/auth/1.0` (asymmetric, no PoP).
   - `/ai-memvault/join/1.0` (token redemption).
   - `ConnectionGater` allowing only `/auth` and `/join` on un-authed connections.
   - `beetswap::Behaviour` on authenticated connections only.
   - gossipsub `ai-memvault/heads/v1` and `ai-memvault/admin/v1`.
5. Cluster genesis tooling: `memctl genesis --admin-key path` produces an initial `trust/admins.cbor` and a `ClusterId`. Admin-keypair generation and rotation primitives.
6. Integration test
   - 5-node in-process cluster with a single admin at genesis.
   - Issue token, new peer redeems, observes their attestation arrives, then connects via `/auth/1.0`.
   - Replay a consumed token → rejected.
   - Rotate the admin key with a 1-hour overlap; new attestation issued by either old or new key during overlap is accepted; after overlap, only new is accepted.
   - Abort an in-flight rotation; verify the rotation is ignored.
   - 1k envelopes round-trip; signatures verify; classification invariants hold.
   - 30% packet loss; convergence.
   - Restart all; persistence.
   - Revoke a peer's attestation mid-flight; new connections refused.

**Definition of done.** 24h soak; replay attempts rejected; classification enforced; audit projection 1:1; one full admin key rotation cycle (issue, overlap, retire) exercised end-to-end.

### Phase 2 — CRDT + history + manual GC

(Unchanged from v5.)

### Phase 3 — Indexes, audit, retraction, agent API, quotas, rotation API

**Goal:** Stable agent API including token issuance, rotation, and renewal flows.

**Deliverables**

1. `memvault-query::index` and `audit` and `retraction`.
2. `memvault-api::quotas`.
3. `memvault-api::tokens`: `issue_token`, `redeem_token`, `revoke_token`, `list_tokens` (with backing audit records).
4. `memvault-api::rotation`: `rotate_admin_key`, `rotate_agent_key`, `abort_rotation`, `renew_attestation`, `list_rotations`.
5. `memvault-api`: full `MemvaultClient` minus `summarize_scope` (stub).
6. `memctl` binary: get, query, put-doc, audit, history, traverse, retract, peers, grants, trust, repair-index, gc, **token issue/list/revoke/redeem**, **rotate-admin-key**, **rotate-agent-key**, **abort-rotation**, **renew-attestation**.

**Definition of done.** End-to-end agent integration. Token lifecycle exercised through CLI. Admin rotates, retires the rotated key, repeats — system stays consistent. Agent rotates its key, then writes a new memory signed with the new key; verifier accepts based on rotation block.

### Phase 4 — Knowledge summarization

(Unchanged from v5.)

### Phase 5 — Classification, PII, egress

(Unchanged from v5.)

### Phase 6 — Cross-cluster federation

Adds: cross-cluster admin rotation propagation via `FederationAnnouncement::AdminKeyRotated`. Federation peers update their `ClusterTrust.trusted_admin_keys` automatically when the partner cluster rotates.

### Phase 7 — Web interface

Adds tokens and rotation surfaces in `AdminPanel`:
- Issue tokens form
- Token list with revoke
- Admin rotation status + abort
- Per-agent rotation history
- Pending attestation renewals queue

### Phase 8 — Operations and hardening

OpenTelemetry. Prometheus. Codec fuzz. Capacity testing. **Multi-admin coordination for token redemption** (no race conditions). **Multi-sig admin operations** (M-of-N for rotations and grants). **Automated rotation** (scheduled via cron-like config; warns operators before key expiry). Cooperative GC. Encrypt-at-rest. FederatedSnapshotApproval. Hard regulatory purge tool. TOML configuration for policy and tags. Custom classification levels. Per-tag-scope quotas. Webapp components 6–10.

---

## 17. Open decisions

| Decision | Default | Revisit at | Notes |
|---|---|---|---|
| Local KV | redb | Phase 1 capacity | sled bugs; rocksdb heavy |
| Multihash | BLAKE3 | Phase 8 | SHA-256 only for interop |
| Snapshot cadence | 1k ops or 5 min | Phase 2 chaos | Per-doc tuning likely |
| Attestation TTL | 24h | Phase 8 | Watch rotation cost |
| Manual vs cooperative GC | manual | Phase 8 | Auto if peers caught-up detection robust |
| Gossipsub topic strategy | per-cluster + filter | Phase 3 | Per-tag if cardinality stable |
| LLM provider | abstract trait | Phase 4 | Caller injects |
| Auth handshake deadline | 10 sec | Phase 8 | Real-world latency |
| Cross-cluster snapshot sharing | not allowed | Phase 8 | FederatedSnapshotApproval |
| Multi-sig admin | single admin | Phase 8 | M-of-N for catastrophic ops |
| Encryption at rest | none | Phase 8 | Only if threat model tightens |
| PII detector backend | regex + Aho-Corasick | Phase 5 | NER feature-gated |
| Custom classification levels | hard-coded 3 | Phase 8 | TOML loader |
| Webapp graph visualization | none | Phase 8 | Consumer-provided if desired |
| Default token TTL | 1 hour | Phase 8 | Tune from real onboarding |
| Default token max-uses | 1 | Phase 8 | Multi-use tokens for batch onboarding |
| Default admin rotation overlap | 7 days | Phase 8 | Long enough for offline peers |
| Default agent rotation overlap | 14 days | Phase 8 | Agents may be even less online |
| Token redemption peer eligibility | any Admin peer | Phase 8 | Single primary admin for high-stakes |
| Token-string format | `mvjoin1:` + base32 cbor | Phase 8 | URL-safe by construction; QR-friendly |

---

## 18. Risks worth tracking

- **beetswap < 1.0** — API churn; isolate behind a thin trait.
- **Loro format stability** — has shipped breaking changes; envelope and snapshot version tags.
- **CRDT log growth without auto-GC** — operators may forget; `memctl peers` shows op-log depth.
- **Auth handshake latency** — every connection pays a round-trip; cache verification.
- **Knowledge-graph cycles** — IPLD DAGs forbid CID cycles; edges resolve via stable IDs through the index.
- **Audit index volume** — every write is an audit record; partition by time bucket.
- **Summarization cost** — LLM calls dominate.
- **Admin key compromise** — catastrophic. Multi-sig is Phase 8. Rotation gives a recovery path: rotate to a new key + revoke the old key's grants.
- **CID leakage breaking discovery-as-access-control** — defense in depth.
- **Wall clock drift** — Lamport authoritative.
- **Federation grant staleness** — short TTLs.
- **Single swarm risk surface** — defense in depth.
- **Retraction is not deletion** — separate purge tool in Phase 8.
- **PII detector false negatives/positives** — confidence scores; feedback loop.
- **LLM redaction is itself egress** — egress check before LLM call.
- **Bearer token theft (webapp)** — short TTLs; CSP.
- **Webapp class-name churn** — `STYLES.md` is the contract.
- **Token leak before consumption** — anyone with the token gets the attached role until consumed/expired/revoked. Mitigate: short TTLs (default 1h), single-use, secure out-of-band sharing, fast revocation. Treat tokens like SSH agent keys.
- **Token redemption race** — two redeems against different admins for `max_uses=1`. Eventual consistency reconciles via gossip; later attestation is revoked. For high-stakes deployments, restrict to a single primary admin (Phase 8).
- **Rotation overlap window misconfiguration** — too short (legitimate signers locked out before propagation completes); too long (compromised key keeps signing). Defaults are conservative (7d admin, 14d agent); tune from operational experience.
- **Rotation race with revocation** — an admin key is rotated and then the new key is immediately compromised. Mitigation: `RotationAborted` works only before `valid_from_ns`; after that, revoke the new key normally. Ensure `valid_from_ns` is at least a few hours in the future to allow detection.
- **Genesis bootstrap problem** — the first admin attestation must be created out-of-band before gossip exists. `memctl genesis` produces the initial trust state; this is a one-time procedure documented in operator docs and audited carefully.
- **Forgotten attestation renewal** — a peer's attestation expires; it can't reconnect. Auto-renewal via cron/timer is the recommended pattern; webapp surfaces upcoming expirations.

---

## 19. First-week tasks

1. Workspace skeleton; `memvault-core`, `memvault-auth`, `memvault-store` crates.
2. `Signed<T>` (version byte, causal+provenance, visibility), canonical DAG-CBOR, sign/verify in `memvault-core`. Property tests.
3. Tag linter enforcing classification structure.
4. `memvault-auth`: `MembershipAttestation` (member==peer invariant), `Grant` (4-action), `Revocation`, `AgentEnrollment`, `ClusterTrust`, **`JoinToken` + `TokenConsumption`**, **`AdminKeyRotation` + `AgentKeyRotation` + `RotationAborted`**, `AdminKeyState` rotation-aware verifier, token-string codec.
5. `memvault-store`: `RedbBlockstore` with 13 index tables (incl. `consumed_tokens` and `rotations`), atomic insert, rotation-state reconstruction on startup.
6. `memvault-net`: `ClusterSwarm` + `/ai-memvault/auth/1.0` (asymmetric, no PoP) + `/ai-memvault/join/1.0` (token redemption) + `ConnectionGater`.
7. Cluster genesis tool: `memctl genesis --admin-key <path>` produces initial trust state.
8. Two-node smoke test: genesis → admin issues token → second peer redeems → second peer connects via /auth → envelope round-trips → admin rotates key → both old and new attestations validate during overlap.

Phase 1 milestone work flows from there.
