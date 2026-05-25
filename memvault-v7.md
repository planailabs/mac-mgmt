# memvault v7 — Buckets, multi-bucket agents, and cross-cluster share mailbox

**Status**: ready-to-execute plan, anchored on the actual `mac-mgmt/memvault` codebase
**Continues from**: `memvault.md` v6 Phase 8 (Operations and hardening) and the reworked attachments track
**New milestone series**: B1–B8 (so the numbers don't collide with the existing Phase 1–8 and A1–A8)

---

## Summary of the change

Today, memvault has clusters (P2P), envelopes (signed, classification-tagged), a knowledge graph (entities, edges, NodeRef), a VFS over the graph, saved Views (read-only tag filters), and one ambient namespace per cluster. Scoping is done by overloading tags (`compartment:archive-only`, `agent:openclaw`, etc.).

We introduce **Bucket** as a first-class scoping primitive that:

1. **Replaces ad-hoc tag-as-scope conventions** for the "main view an agent operates in." Tags stay for classification/kind/handling/agent; bucket is its own slot.
2. **Lifecycle: local → attached → shared.** Agents can create a bucket privately on their peer, work in it, then attach it to the cluster (gossip flows), then optionally share it with another cluster (via an approval mailbox).
3. **Multi-bucket access for one agent** with a designated `default_bucket` for unscoped ops, plus enumerable `read/write` grants on additional buckets.
4. **Cross-cluster sharing requires explicit agent approval** via a new mailbox protocol (`/ai-memvault/share/1.0`) reviewed via MCP tools.
5. **Views are reframed** as in-bucket browsing filters. A view either targets one bucket (the common case) or fans out across the agent's accessible buckets.
6. **Sync is layered, not replaced** — buckets ride on the existing bitswap + gossipsub + Loro stack with a private-bucket gossip filter and a serve-time access predicate. See the Sync section for the full consistency model.

All changes are additive: v1 envelopes without a bucket reference are treated as belonging to the cluster's `default_bucket`, which is created at genesis. No breaking changes to existing data.

---

## Data shapes (changes to `memvault-core` and `memvault-auth`)

### `BucketId` (new, in `memvault-core::ids`)

```rust
/// Bucket identifier — random 32 bytes, like ClusterId/DocId.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BucketId(pub [u8; 32]);

impl BucketId {
    pub fn random() -> Self {
        let mut buf = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut buf);
        Self(buf)
    }
}

impl std::fmt::Display for BucketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", bs58::encode(&self.0).into_string())
    }
}
```

### `BucketDecl` and bucket-related CRDT ops (new, in `memvault-doc::op`)

```rust
/// Payload of a Signed<BucketDecl> envelope. A bucket is declared by writing
/// this; membership is by tag (`bucket:<bs58>`) on every other envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketDecl {
    pub bucket_id: BucketId,
    pub name: String,
    pub description: Option<String>,
    pub owner_agent: Option<AgentId>,        // None = cluster-owned (the default bucket)
    pub default_visibility: Visibility,
    pub default_classification: Classification,
    pub is_default: bool,                    // true on exactly one bucket per cluster
    pub created_ns: u64,
    /// If Some, the bucket is local-only to this peer until attached.
    pub private_to_peer: Option<PeerId>,
}

// Extend the existing Op enum (do not break existing variants — append only).
pub enum Op {
    // ... existing variants ...
    BucketCreate   { decl: BucketDecl },
    BucketRename   { bucket_id: BucketId, new_name: String },
    BucketArchive  { bucket_id: BucketId, reason: String },
    /// Flips private_to_peer to None and gossips heads to the cluster.
    BucketAttach   { bucket_id: BucketId, attached_at_ns: u64 },
    /// Records membership grants to additional agents (audit + replication signal).
    BucketGrantAgent { bucket_id: BucketId, agent: AgentId, actions: Vec<Action> },
    BucketRevokeAgent { bucket_id: BucketId, agent: AgentId },
}
```

The envelope wrapping a `BucketDecl` op carries the tag `bucket:<bs58>` like any other bucket-scoped envelope, plus the reserved tag `kind:bucket-decl`. This means the bucket appears in `BY_TAG` queries naturally — no new code paths.

### `GrantScope` (new, extends `Grant` in `memvault-auth`)

```rust
/// A scope that a grant applies to. Replaces the old `scopes: Vec<TagPattern>`
/// field, keeping the tag-pattern variant for backwards compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GrantScope {
    Bucket(BucketId),
    Tag(TagPattern),
    /// Any bucket and any tag (legacy admin grants).
    Any,
}

pub struct Grant {
    pub issuer: PeerId,
    pub issuing_cluster: ClusterId,
    pub audience: GrantAudience,
    /// Scopes are OR-ed: the grant applies if any scope matches the access.
    pub scopes: Vec<GrantScope>,
    pub actions: Vec<Action>,
    pub not_before_ns: u64,
    pub not_after_ns: u64,
    pub parent: Option<Cid>,
    pub nonce: [u8; 16],
    pub signature: [u8; 64],
}
```

Old grants with `scopes: Vec<TagPattern>` deserialize via a custom `Deserialize` impl that maps each pattern to `GrantScope::Tag(pattern)`. The pattern follows the precedent set by `NodeRef::Deserialize`.

### `AgentEnrollment` (extend in `memvault-auth::enrollment`)

```rust
pub struct AgentEnrollment {
    pub agent_id: AgentId,
    pub public_key: PublicKey,
    pub cluster_id: ClusterId,
    pub enrolled_by: PeerId,
    pub initial_grants: Vec<Cid>,
    /// NEW: bucket the agent writes to when no bucket is specified on a call.
    /// Defaults to the cluster's default bucket if not set.
    pub default_bucket: Option<BucketId>,
    pub not_after_ns: u64,
    pub signature: [u8; 64],
}
```

### Tag linter rule (extend `memvault-core::tags_lint`)

```rust
pub fn lint_tags(tags: &[Tag]) -> Result<(Classification, Option<BucketId>)> {
    let classification = /* existing rule: exactly one classification:<level> */;
    let bucket = match tags.iter().filter(|t| t.scope == "bucket").count() {
        0 => None,                                  // belongs to default bucket
        1 => {
            let t = tags.iter().find(|t| t.scope == "bucket").unwrap();
            Some(parse_bucket_label(&t.label)?)      // bs58-decoded
        }
        n => return Err(Error::TagLint(format!(
            "expected at most one bucket tag, found {n}"))),
    };
    Ok((classification, bucket))
}
```

### Cross-cluster share types (new, in `memvault-auth::share` and `memvault-net::share_proto`)

```rust
pub struct ShareProposal {
    pub proposal_id: [u8; 16],
    pub from_cluster: ClusterId,
    pub from_bucket: BucketId,
    pub from_admin: PeerId,                  // proposer
    pub to_cluster: ClusterId,
    pub to_recipient: ShareRecipient,        // who can approve
    pub proposed_actions: Vec<Action>,       // typically [Read]
    pub proposed_visibility: Visibility,
    pub purpose: String,                     // human-readable; shown in MCP
    pub not_after_ns: u64,                   // proposal expiry
    pub signature: [u8; 64],
}

pub enum ShareRecipient {
    /// Any peer in the target cluster holding Action::Admin.
    AnyAdmin,
    /// A specific agent must approve (e.g. the "sharing-agent" running the MCP).
    Agent(AgentId),
}

pub enum ShareDecision {
    Approve { granted_actions: Vec<Action>, not_after_ns: u64 },
    Reject  { reason: String },
    NeedsHumanReview { staffed_at_ns: u64 },
}

pub struct ShareReply {
    pub proposal_id: [u8; 16],
    pub from_cluster: ClusterId,             // the cluster that decided
    pub by_principal: PeerId,                // agent or admin who decided
    pub decision: ShareDecision,
    pub decided_at_ns: u64,
    pub signature: [u8; 64],
}

/// Signed grant issued by the approving cluster after approval. This is what
/// actually opens up federation reads on the shared bucket.
pub struct BucketTrust {
    pub bucket_id: BucketId,
    pub from_cluster: ClusterId,             // owner cluster (proposer side)
    pub to_cluster: ClusterId,               // receiving cluster
    pub actions: Vec<Action>,
    pub not_after_ns: u64,
    pub from_proposal: Cid,                  // ShareProposal CID
    pub from_reply: Cid,                     // ShareReply CID
    pub signature: [u8; 64],                 // by receiving cluster's admin
}
```

`BucketTrust` is the per-bucket equivalent of the existing `ClusterTrust`. The `AuthVerifier` is extended to accept federation reads on a bucket if either (a) a `ClusterTrust` exists (legacy whole-cluster trust) OR (b) a valid `BucketTrust` covers the bucket+action+time.

---

## Storage changes (in `memvault-store`)

### New tables

```rust
/// Bucket index: packed(bucket_id, wall_ns, cid) -> ().
pub const BY_BUCKET: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("by_bucket");

/// Bucket metadata: bucket_id -> bucket_decl_cid (most-recent BucketDecl op).
pub const BUCKETS: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("buckets");

/// Bucket → cluster: bucket_id -> cluster_id (allows cross-cluster bucket discovery).
pub const BUCKET_CLUSTER: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("bucket_cluster");

/// Inbox for share proposals: packed(to_cluster, wall_ns, proposal_cid) -> ().
pub const SHARE_INBOX: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("share_inbox");

/// Outbox for share proposals: packed(from_cluster, wall_ns, proposal_cid) -> status_byte.
/// status: 0 = pending, 1 = approved, 2 = rejected, 3 = expired.
pub const SHARE_OUTBOX: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("share_outbox");

/// Cross-cluster bucket trust: packed(bucket_id, from_cluster, to_cluster) -> trust_cid.
pub const BUCKET_TRUST: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("bucket_trust");
```

### `EnvelopeMeta` extension

```rust
pub struct EnvelopeMeta {
    pub author: Vec<u8>,
    pub tags: Vec<(String, String)>,
    pub wall_ns: u64,
    pub causal: Vec<Vec<u8>>,
    pub provenance: Vec<Vec<u8>>,
    pub cluster_id: Option<Vec<u8>>,
    pub bucket_id: Option<Vec<u8>>,      // NEW — extracted from the bucket: tag
}
```

`insert_envelope` extracts the bucket tag at index time and writes to `BY_BUCKET`. No envelope schema change, no new field on `Signed<T>` — bucket membership rides on the existing tag set, but is **promoted to a first-class index slot** so queries don't have to filter at scan time.

### Query helpers (in `memvault-store::query`)

```rust
impl MemvaultStore {
    pub fn query_by_bucket(
        &self,
        bucket_id: &[u8],
        after_ns: u64,
        limit: usize,
    ) -> Result<Vec<Vec<u8>>, StoreError> { /* range scan on BY_BUCKET */ }

    pub fn list_buckets(&self) -> Result<Vec<(BucketId, Vec<u8>)>, StoreError> {
        // Returns (bucket_id, bucket_decl_cid) pairs.
    }

    pub fn get_default_bucket(&self) -> Result<Option<BucketId>, StoreError> {
        // Reads BUCKETS, follows BucketDecl with is_default=true.
    }

    pub fn record_share_inbox(&self, proposal_cid: &[u8], to_cluster: &[u8], wall_ns: u64) -> Result<(), StoreError>;
    pub fn pending_share_proposals(&self, to_cluster: &[u8]) -> Result<Vec<Vec<u8>>, StoreError>;
    pub fn record_bucket_trust(&self, trust_cid: &[u8], bucket_id: &[u8], from: &[u8], to: &[u8]) -> Result<(), StoreError>;
}
```

---

## API surface (changes to `memvault-api::MemvaultClient`)

```rust
#[async_trait]
pub trait MemvaultClient: Send + Sync {
    // ── Buckets ──────────────────────────────────────────────────────
    /// Create a new bucket. Defaults to private-to-this-peer.
    async fn bucket_create(
        &self,
        name: &str,
        description: Option<&str>,
        default_visibility: Visibility,
        default_classification: Classification,
        owner_agent: Option<&AgentId>,
    ) -> Result<BucketId>;

    /// List all buckets visible to the caller.
    async fn bucket_list(&self) -> Result<Vec<BucketInfo>>;

    /// Get a single bucket's metadata.
    async fn bucket_get(&self, id: &BucketId) -> Result<Option<BucketInfo>>;

    /// Set the agent's default bucket (writes a new AgentEnrollment).
    async fn bucket_set_default(&self, agent: &AgentId, bucket: &BucketId) -> Result<()>;

    /// Attach a private bucket to the cluster (publishes heads, makes visible to peers).
    async fn bucket_attach(&self, id: &BucketId) -> Result<()>;

    /// Archive a bucket (soft-removes from active list; data and audit preserved).
    async fn bucket_archive(&self, id: &BucketId, reason: &str) -> Result<()>;

    /// Grant an agent access to a bucket.
    async fn bucket_grant_agent(
        &self,
        bucket: &BucketId,
        agent: &AgentId,
        actions: Vec<Action>,
    ) -> Result<Cid>;

    // ── Cross-cluster sharing ────────────────────────────────────────
    /// Propose to share a bucket with another cluster. Returns the proposal CID
    /// so the caller can poll status.
    async fn share_propose(
        &self,
        bucket: &BucketId,
        to_cluster: &ClusterId,
        recipient: ShareRecipient,
        proposed_actions: Vec<Action>,
        purpose: &str,
        ttl_secs: u64,
    ) -> Result<Cid>;

    /// List share proposals received by this cluster, optionally filtered.
    async fn share_inbox(&self, only_pending: bool) -> Result<Vec<ShareProposal>>;

    /// List share proposals this cluster sent out.
    async fn share_outbox(&self) -> Result<Vec<(ShareProposal, ShareStatus)>>;

    /// Decide a share proposal (approve or reject). Records a ShareReply and,
    /// on approval, issues a Signed<BucketTrust>.
    async fn share_decide(
        &self,
        proposal_cid: &Cid,
        decision: ShareDecision,
    ) -> Result<()>;

    // ── Existing methods now accept an optional bucket override ──────
    async fn put_doc(
        &self,
        doc: Document,
        tags: Vec<(String, String)>,
        vis: Visibility,
        bucket: Option<BucketId>,                  // None = agent's default bucket
    ) -> Result<Vec<u8>>;
    // (upload_file, add_entity, add_link, etc. get the same trailing param)
}

pub struct BucketInfo {
    pub id: BucketId,
    pub name: String,
    pub description: Option<String>,
    pub owner_agent: Option<AgentId>,
    pub is_default: bool,
    pub is_attached: bool,                         // private_to_peer is None
    pub default_visibility: Visibility,
    pub default_classification: Classification,
    pub created_ns: u64,
    pub block_count: u64,
}
```

**Migration tactic for write methods.** Adding a parameter to a heavily-used trait method is painful. Two options:

- **(a)** Add the parameter as the last position with `Option<BucketId>`, updating all call sites.
- **(b)** Add a sibling builder API `put_doc_into(bucket).tags(...).submit(doc)` and keep `put_doc` as `put_doc_into(default).submit(...)`.

Recommend **(a)** for the v7 cutover — it's a one-shot mechanical refactor that the compiler enforces. The `Option` allows callers to keep passing `None` (which means "agent's default bucket").

---

## Network protocols (in `memvault-net`)

### `/ai-memvault/share/1.0` (new request-response protocol)

```rust
pub const SHARE_PROTOCOL: StreamProtocol = StreamProtocol::new("/ai-memvault/share/1.0");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRequest {
    pub version: u8,                       // = 1
    pub proposal_block: Vec<u8>,           // serialized Signed<ShareProposal>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareResponse {
    pub version: u8,
    pub result: ShareResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShareResult {
    /// Proposal was accepted into the inbox; decision is async.
    Queued { proposal_cid: Vec<u8> },
    /// Receiving peer does not hold a recipient-eligible identity.
    NotRecipient { try_peers: Vec<PeerAddress> },
    /// Proposal failed validation (bad signature, expired, etc.).
    Invalid { reason: String },
    /// Cluster has not authorized cross-cluster sharing.
    Refused { reason: String },
}
```

This is the second protocol (after `/ai-memvault/join/1.0`) allowed on connections that have *not yet* completed `/auth/1.0` — because the proposer's cluster may not yet have any attestation in the receiving cluster. The receiver still verifies the proposal's signature against the proposer's cluster admin keys (looked up via `ClusterTrust` if any exists, or out-of-band if first contact).

For **first-contact** between two clusters with no prior `ClusterTrust`: the receiver accepts the proposal into the inbox marked `provisional_first_contact = true`. The reviewing agent sees this flag in MCP and uses extra caution.

### Gossip extension (in `memvault-net::gossip`)

```rust
pub enum AdminAnnouncement {
    TokenConsumed(Cid),
    AdminKeyRotated(Cid),
    AgentKeyRotated(Cid),
    RotationAborted(Cid),
    Revoked(Cid),
    // NEW
    BucketCreated(Cid),         // BucketDecl envelope CID
    BucketAttached(Cid),        // BucketAttach op CID
    BucketArchived(Cid),
    BucketTrustEstablished(Cid),  // BucketTrust envelope CID
}

pub enum FederationAnnouncement {
    HeadAvailable { head_cid: Vec<u8>, visibility: Visibility, scope_tags: Vec<(String, String)>, bucket_id: Option<Vec<u8>> },   // bucket_id is NEW
    GrantIssued { grant_cid: Vec<u8> },
    GrantRevoked { revocation_cid: Vec<u8> },
    TrustRevoked { revocation_cid: Vec<u8> },
    AdminKeyRotated { rotation_cid: Vec<u8> },
    // NEW
    BucketTrustEstablished { trust_cid: Vec<u8> },
    BucketTrustRevoked { revocation_cid: Vec<u8> },
}
```

---

## Sync and consistency

Buckets do not introduce a new sync mechanism — they layer filters on top of the three existing ones. This section pins down exactly how sync flows through bucket lifecycle transitions and what consistency the system guarantees.

### The three existing sync layers

| Layer | Mechanism | Role |
|---|---|---|
| **Block bytes** | `beetswap` (libp2p bitswap) on authenticated connections | Pull-based block exchange. Any authed peer can request any CID; serving peer applies visibility + bucket-trust checks at serve time. |
| **Metadata pointers** | `gossipsub` on `ai-memvault/heads/v1`, `ai-memvault/admin/v1`, federation topics | Push-based fan-out of "head X exists" and "admin event Y happened". Tiny messages — never the data itself. |
| **Per-doc convergence** | Loro CRDT applied to `Signed<Op>` envelopes | Operations are content-addressed and signed; replaying them in causal order on any peer yields bit-identical document state. |

Three ordering primitives on every envelope:
- **Lamport clocks** (`wall_ns` is recorded for UX but never trusted for correctness — Lamport is authoritative).
- **`causal: Vec<Cid>`** — explicit CRDT parents on ops, empty on non-op envelopes.
- **`provenance: Vec<Cid>`** — semantic ancestry, separate from causal.

### Four sync paths bucket lifecycle exercises

#### Path 1 — Private bucket (no sync)

```
peer_A           peer_B (same cluster)         cluster A's gossip
  │                  │                                │
  │ BucketCreate     │                                │
  │  (private_to_peer = peer_A)                       │
  │                  │                                │
  │ ── 100 doc ops in bucket X ──┐                    │
  │                              │                    │
  │ filter at gossip publish:    │                    │
  │   "is bucket X's BucketDecl  │                    │
  │    private_to_peer == self?  │                    │
  │    → don't publish heads"    │                    │
  │                  │                                │
  │ filter at bitswap-serve:     │                    │
  │   "is requested CID in       │                    │
  │    a private-to-someone-else │                    │
  │    bucket? → refuse"         │                    │
  │                  │                                │
  │ (peer_B sees nothing)        │                    │
```

The BucketDecl itself is **never gossiped** while private. Two filters enforce this:

- `memvault-net::gossip` **publish-side**: skip heads for envelopes tagged `bucket:X` if the local BucketDecl for X has `private_to_peer = Some(_)`.
- `memvault-net::bitswap` **serve-side**: refuse to serve any CID whose envelope's bucket has `private_to_peer = Some(other)` regardless of who's asking. This is the safety net — if gossip leaks a CID during a race, serve still refuses.

#### Path 2 — Attached bucket (intra-cluster sync)

```
peer_A                 cluster gossip            peer_B / peer_C / ...
  │                          │                        │
  │ BucketAttach op          │                        │
  │  (writes new BucketDecl  │                        │
  │   with private_to_peer   │                        │
  │   = None)                │                        │
  │                          │                        │
  │ ── AdminAnnouncement::BucketAttached(CID) ──────► │
  │                          │                        │
  │ ── Heads for every CID in bucket X ─────────────► │
  │                          │                        │
  │                          │                  peer_B sees heads,
  │                          │                  asks bitswap for each CID
  │ ◄────── bitswap WANT ─── │ ─────────────────── │
  │ ────── bitswap HAVE ──── │ ──────────────────► │
  │ ────── bitswap BLOCK ─── │ ──────────────────► │
  │                          │                        │
  │                          │              peer_B re-indexes:
  │                          │                bucket:X tag → BY_BUCKET
  │                          │                BucketDecl → BUCKETS
  │                          │                (envelope) → BY_TAG, BY_AUTHOR, BY_TIME
```

Convergence here rests on two things:

1. **Heads are idempotent.** Receiving the same head announcement twice produces the same fetch. Gossipsub may deliver duplicates; the receiver checks "do I already have this CID?" via a blockstore lookup before issuing a WANT.
2. **BucketAttach is a CRDT op.** It's a `Signed<Op::BucketAttach>` with `causal: [previous_BucketDecl_cid]`. Two peers can't disagree about whether a bucket is attached — they either have the attach op or they don't.

Back-pressure: a peer can decline to fetch a bucket via config (e.g. `bucket_pin_policy = "selective"` plus an explicit pin list). This reuses the same dial that already exists for eager attachments in `memvault-attach`.

#### Path 3 — Share proposal/reply (cross-cluster metadata exchange)

This one is **synchronous request-response over `/share/1.0`**, not gossip — by design, because proposals need a delivery guarantee and a clear "did this reach the inbox?" answer for the proposer.

```
cluster A                /share/1.0           cluster B
proposer admin               ┌─────────┐      receiver (any auth'd peer)
  │                          │         │      │
  │ ShareProposal signed     │ TLS +   │      │
  │ by A's admin key         │ libp2p  │      │
  │                          │ noise   │      │
  │ ── ShareRequest ────────►│         │ ────►│
  │                          │         │      │ verify signature against
  │                          │         │      │ proposer cluster's admin
  │                          │         │      │ keys (from ClusterTrust
  │                          │         │      │ or first-contact flag);
  │                          │         │      │ check not-after; check
  │                          │         │      │ rate limit; write to
  │                          │         │      │ SHARE_INBOX
  │                          │         │      │
  │ ◄── ShareResponse::Queued ──────── │ ◄───│
  │     { proposal_cid }     │         │      │
  │                          │         │      │
  │ write SHARE_OUTBOX       │         │      │ AdminAnnouncement
  │ (status = pending)       │         │      │ ::ShareProposalReceived
  │                          │         │      │ broadcast within cluster B
  │                          │         │      │
  │                          │         │      │ ── triggers MCP push to
  │                          │         │      │     share-agent (or web UI)
```

The **reply** flows back symmetrically: cluster B's share-agent issues `Signed<ShareReply>` and (on approval) `Signed<BucketTrust>`, then opens a fresh `/share/1.0` stream **outbound to cluster A** to deliver them. Cluster A persists both, updates SHARE_OUTBOX status, and starts gossiping the bucket on the federation topic.

The proposal flow is request-response so the proposer gets an explicit `Queued` ack and knows whether to retry. This is unlike heads gossip (fire-and-forget).

Firewall asymmetry caveat: if cluster B is outbound-only, B initiates both legs. The receiving cluster A then has nothing to poll *to*; A just waits. For purely inbound-only B (rare), the proposer polls — a v8 nuance, not B6 scope.

#### Path 4 — Shared bucket data (cross-cluster sync)

After `Signed<BucketTrust>(from=A, to=B, bucket=X, actions=[Read])` is persisted on both sides:

```
cluster A                  federation gossip       cluster B
  │                              │                    │
  │ for each CID in bucket X:    │                    │
  │   FederationAnnouncement     │                    │
  │   ::HeadAvailable {          │                    │
  │     head_cid,                │                    │
  │     visibility: Federated,   │                    │
  │     bucket_id: X,            │                    │
  │   }                          │                    │
  │ ───────────────────────────► │ ─────────────────► │
  │                              │                    │
  │                              │              B's gossip handler:
  │                              │                check visibility ≠ Internal
  │                              │                check bucket_id ∈
  │                              │                  bucket_trusts_for(A → B)
  │                              │                if both ok → enqueue fetch
  │                              │                    │
  │ ◄── bitswap WANT ─────────── │ ────────────────── │
  │                              │                    │
  │ A's serve handler:           │                    │
  │   peer in remote cluster B?  │                    │
  │   envelope visibility ≠ Internal?                 │
  │   envelope bucket_id has valid BucketTrust(A→B)?  │
  │   all yes → serve            │                    │
  │ ── block bytes ───────────── │ ─────────────────► │
```

The **serve-side check** is the authoritative gate — see "Serve-side check" below.

### Consistency guarantees

memvault provides **three layered guarantees**, none of which are linearizability:

#### Guarantee 1 — Strong Eventual Consistency on document state (Loro CRDT)

For any document `D` and any two peers `P1`, `P2` that have received the same set of `Signed<Op>` envelopes for `D`, the result of `apply_doc_ops` on each is **bit-identical**. This is the Loro CRDT contract. Concurrent edits by two writers converge to the same state regardless of delivery order, as long as causal precedence (`causal: Vec<Cid>`) is respected.

In the bucket world: two agents both writing to the same doc inside a shared bucket, even from different clusters, converge. There is no "edit conflict" surface — it's a CRDT.

#### Guarantee 2 — Causal consistency on op ordering

Every op envelope carries `causal: Vec<Cid>` (CRDT parents) and `lamport: u64`. A peer never applies an op until all its causal parents are present. So:

- "User added paragraph A, then deleted it" → never observed as "delete then add"
- "Bucket created, then attached, then doc written" → never observed in a different order

Lamport is the deterministic tiebreaker for concurrent ops (same causal set, different writers). Two peers seeing the same op set produce the same total order.

#### Guarantee 3 — Monotonic, append-only audit log

Every write is a signed envelope with a stable CID. Once a peer indexes a CID, it never unindexes. Retraction is a *new* envelope (a tombstone) — the retracted envelope's bytes remain. This is why `memctl repair-index` can rebuild deterministically from BLOCKS alone.

The audit log isn't strongly ordered across peers (different peers see different subsets at different times), but **per-CID facts are immutable** once committed. You can ask "what does peer P believe about CID X right now?" and get a stable answer until garbage collection.

#### What you don't get

- **No linearizability.** "Bucket archive happens before doc write" is not enforceable across peers.
- **No global ordering across clusters.** Lamport is per-cluster; cross-cluster ordering uses `wall_ns` as a hint only.
- **No cross-bucket transactions.** Writing doc D1 to bucket X and doc D2 to bucket Y is two independent writes.
- **No read-your-writes across peers.** Writing to peer A and immediately reading on peer B may return the old state until gossip + bitswap complete (typically <1s on a healthy cluster — no SLA).

### Race resolutions

| Race | What can happen | How it resolves |
|---|---|---|
| **Bucket attach + concurrent in-flight private write** | Peer A is writing doc 99 in bucket X *while* issuing BucketAttach. Doc 99's envelope reaches the blockstore after the attach. | Doc 99 is in the bucket like all others. The attach gossip publishes its CID along with the existing 98. Convergent — no special handling needed. The attach op's Lamport is just one of the bucket's ops. |
| **Bucket archive + concurrent write** | Peer A archives bucket X. Peer B, not yet seeing the archive, writes doc 100 into X. | Doc 100 is committed (valid envelope). Once peer B receives BucketArchive (Lamport > doc 100's), it indexes doc 100 normally but also indexes the archive. Reads on peer B see doc 100 with a "in archived bucket" banner. Same pattern as retraction. |
| **Two admins approve the same proposal** | Cluster B has two admins; both run `share approve` before gossip reconciles. Both sign `ShareReply` and `BucketTrust`. | Both reach cluster A. SHARE_OUTBOX deduplicates by `proposal_id` and picks the **earliest reply** by `(lamport, signer_peer_id)` — same algorithm as `TokenConsumption` reconciliation in the v6 design. The later reply is kept for audit, marked superseded. The verifier picks the earliest valid trust. |
| **BucketTrust revocation + in-flight bitswap** | Cluster B's admin revokes trust mid-stream. Cluster A is serving block 47 of 100. | The revocation arrives on the admin gossip topic; the next per-block serve check fails. Block 47's transfer completes (TCP doesn't unsend); 48–100 are refused. Cluster B has a partial bucket — surfaced via the bucket-completeness Bloom-filter heartbeat (B8). |
| **Concurrent bucket rename** | Two grantees rename bucket X within the same Lamport step. | LWW by `(lamport, signer_peer_id)`. Same pattern Loro uses for non-text properties — bucket names live in a Loro map for exactly this reason. |
| **Two peers create different buckets with the same name** | Peer A and peer B both run `bucket new "research"` while partitioned. | No conflict. BucketId is random 32 bytes — different IDs. Both coexist. The name is a label, not an identifier. CLI/web UI shows them with a (1)/(2) disambiguator. |
| **First-contact share proposal, no prior ClusterTrust** | Cluster A proposes to cluster B for the first time. B has no idea who A is. | Inbox row marked `provisional_first_contact = true`. The signature is verified against A's admin key carried in the proposal. Trusting that key is the reviewer's call. Approval implicitly adds A to ClusterTrust (atomically with issuing BucketTrust). TOFU pattern from v6 doc. |
| **BucketTrust gossip arrives but BucketDecl hasn't** | Cluster B receives `BucketTrustEstablished(trust_cid)` referencing bucket X, but A hasn't yet gossiped the BucketDecl. | Cluster B queues the trust as "pending bucket discovery", retries fetching the BucketDecl via bitswap, and only enters the trust into BUCKET_TRUST once the decl is present. Same pattern as `Grant` with `parent: Option<Cid>`. |

### The serve-side check is the safety net

All the consistency claims above rest on one assumption: **every peer that serves a block evaluates the access predicate at serve time, not at gossip time.** The gossip filter is a performance optimization (don't bother peers with heads they can't read); the serve filter is the security boundary.

Even if `BucketTrust` propagation is slow, even if a peer's view of admin keys is stale, even if revocations are in flight — the **last evaluation** before bytes leave the wire is authoritative.

```rust
fn may_serve(
    &self,
    cid: &Cid,
    requester_peer: &PeerId,
    requester_cluster: &ClusterId,
    now_ns: u64,
) -> Result<(), ServeError> {
    let envelope = self.store.get_envelope(cid)?
        .ok_or(ServeError::NotFound)?;
    let meta = self.store.envelope_meta(cid)?;

    // 1. Visibility check (existing).
    if envelope.visibility == Visibility::Internal
        && requester_cluster != &self.local_cluster
    {
        return Err(ServeError::VisibilityRefused);
    }

    // 2. Private bucket check (NEW).
    if let Some(bucket_id) = &meta.bucket_id {
        let decl = self.store.bucket_decl(bucket_id)?
            .ok_or(ServeError::BucketUnknown)?;
        if let Some(owner_peer) = &decl.private_to_peer {
            if owner_peer != &self.local_peer_id {
                // Private to someone else on this same cluster.
                return Err(ServeError::BucketPrivate);
            }
            if requester_peer != &self.local_peer_id {
                // Private to me — refuse to anyone else.
                return Err(ServeError::BucketPrivate);
            }
        }
    }

    // 3. Cross-cluster bucket-trust check (NEW).
    if requester_cluster != &self.local_cluster {
        match &meta.bucket_id {
            Some(bucket_id) => {
                let trust = self.store.bucket_trust_for(
                    bucket_id, &self.local_cluster, requester_cluster, now_ns,
                )?;
                let allowed = trust
                    .as_ref()
                    .map(|t| t.actions.contains(&Action::Read))
                    .unwrap_or(false);
                if !allowed {
                    return Err(ServeError::NoBucketTrust);
                }
            }
            None => {
                // Default-bucket envelopes fall back to whole-cluster ClusterTrust.
                if !self.has_cluster_trust(requester_cluster, now_ns)? {
                    return Err(ServeError::NoClusterTrust);
                }
            }
        }
    }

    // 4. Revocation check (existing).
    if self.store.is_revoked(cid)? {
        return Err(ServeError::Revoked);
    }

    Ok(())
}
```

This function runs **on every block served**. Every lookup is a point read on an indexed redb table — cheap. It's the place where every consistency question ultimately gets answered.

### Operational tools for observing sync

Ship alongside B1–B8:

- **`memctl bucket peers <bucket_id>`** — per-peer count of how many bucket CIDs each peer has. Spots stuck replication.
- **`memctl share status <proposal_cid>`** — outbox status, reply if received, resulting BucketTrust CID.
- **`memctl federation status`** — every cross-cluster trust, when issued, last gossip-heartbeat from the federated cluster.
- **A new chaos-test scenario `sim-tests/scenarios/bucket_partition.rs`** — runs the existing fault-injector with a 3-node cluster, partitions one node during a bucket attach, heals the partition, asserts convergence within N seconds. Narrowing of the v6 "30% packet loss; convergence" test to buckets.
- **Federation gossip-loss metric** — count of `BucketTrustEstablished` announcements received vs. trust CIDs persisted, exported via OpenTelemetry per v6 Phase 8.

---

## MCP server additions (`plan-ai-memvault`)

New tools, additive to the existing 43:

| Tool | Description |
|---|---|
| `memvault_bucket_create` | Create a new bucket (private-to-peer by default). |
| `memvault_bucket_list` | List buckets visible to the calling agent. |
| `memvault_bucket_get` | Get a bucket's metadata by ID. |
| `memvault_bucket_attach` | Attach a private bucket to the cluster. |
| `memvault_bucket_archive` | Archive a bucket. |
| `memvault_bucket_set_default` | Set agent's default bucket. |
| `memvault_bucket_grant_agent` | Grant another agent access to a bucket. |
| `memvault_share_propose` | Send a cross-cluster share proposal. |
| `memvault_share_inbox` | List proposals received from other clusters. |
| `memvault_share_outbox` | List proposals this cluster sent. |
| `memvault_share_approve` | Approve a pending share proposal. |
| `memvault_share_reject` | Reject a pending share proposal. |

Existing write tools (`memvault_put`, `memvault_upload_file`, `memvault_graph_add`, `memvault_link`, etc.) gain an optional `bucket` parameter (bs58 ID). If absent, the agent's default bucket is used. The MCP server already takes a `MEMVAULT_DEFAULT_TAGS` env var; we add `MEMVAULT_DEFAULT_BUCKET` for symmetry.

Read tools (`memvault_search`, `memvault_list`, `memvault_audit`, `memvault_traverse`, `memvault_vfs_ls`) gain an optional `bucket` filter. Without it they search across all buckets the agent has read access to.

### A new MCP server: `plan-ai-memvault-share-agent`

A focused, low-tool-count MCP server for an **approval agent** to run in a target cluster. Tool list:

| Tool | Description |
|---|---|
| `share_inbox_list` | All pending proposals, with cluster/bucket/purpose/proposer summary. |
| `share_inspect` | Deep dive on one proposal: signature chain, cluster trust history, bucket sample content if read-allowed. |
| `share_check_classifications` | Run policy: what classifications are present in the source bucket; do any exceed the receiver's outbound rules? |
| `share_approve` | Approve with optional attenuation (smaller action set, shorter TTL). |
| `share_reject` | Reject with a reason. |
| `share_staff_ping` | Escalate to a human (re-uses the `mac-mgmt-healer` staff-ping subsystem). |

This MCP server is the natural place to integrate Claude (or any LLM) as a sharing reviewer. It is intentionally siloed from the broader memvault MCP so it can run under its own identity with its own grants.

---

## CLI surface (`memctl`)

New subcommands:

```
memctl bucket new <name> [--desc TEXT] [--owner AGENT] \
                         [--visibility internal|federated|public] \
                         [--classification public|internal|confidential]
memctl bucket list
memctl bucket show <bucket-id>
memctl bucket attach <bucket-id>
memctl bucket archive <bucket-id> --reason "TEXT"
memctl bucket set-default <agent-id> <bucket-id>
memctl bucket grant <bucket-id> <agent-id> --actions read,write

memctl share propose <bucket-id> --to-cluster <hex> \
                                 [--recipient any-admin|agent:<id>] \
                                 [--actions read,write] \
                                 --purpose "TEXT" \
                                 [--ttl 24h]
memctl share inbox [--pending]
memctl share outbox
memctl share approve <proposal-cid> [--actions read]
memctl share reject <proposal-cid> --reason "TEXT"
```

The agent-key rotation and token primitives remain unchanged.

---

## Phase plan

Each phase is a milestone. The "Files" line lists where the change lands; the "Acceptance" line is what `cargo test` and the smoke tests must show before the next phase starts.

### B1 — Bucket primitive and storage (≈ 6 dev-days) ★ milestone

**Goal**: Bucket exists as a typed primitive; envelopes can be tagged with a bucket; the BY_BUCKET index is populated; default bucket is created at genesis.

Tasks:
- `memvault-core`: add `BucketId`; extend `tags_lint` to recognize at-most-one `bucket:<bs58>` tag and parse it.
- `memvault-doc::op`: add `Op::BucketCreate/Rename/Archive/Attach/GrantAgent/RevokeAgent`. Add `BucketDecl` to `memvault-doc::bucket` (new module).
- `memvault-store`: add `BY_BUCKET`, `BUCKETS`, `BUCKET_CLUSTER` tables; extend `EnvelopeMeta` with `bucket_id`; index on insert; add `query_by_bucket`, `list_buckets`, `get_default_bucket`.
- `memvault-api`: add `bucket_create`, `bucket_list`, `bucket_get`, `bucket_set_default` to `MemvaultClient`; implement in `LocalClient`. Add `BucketInfo`.
- `memvault-api::local`: cluster genesis path now writes a `Signed<BucketDecl>` with `is_default=true, private_to_peer=None, name="default", owner_agent=None` before any agent enrollment.
- `memctl`: `bucket new`, `bucket list`, `bucket show`, `bucket set-default`.
- Tests in `memvault/tests/integration/buckets_b1.rs`: create three buckets, list them, query envelopes by bucket, confirm BY_BUCKET index hits are O(1) per bucket-CID lookup, confirm classification linter rejects `bucket:` + two-bucket-tags envelopes.

Acceptance:
- Two-peer test: peer A creates a private bucket, writes 100 docs into it. Peer B (same cluster) does **not** see any of them yet. `bucket list` on peer B shows no extra bucket.
- New cluster genesis now produces a default bucket; `memctl bucket list` shows it; envelopes without an explicit bucket tag land in the default bucket index.

### B2 — Bucket-aware grants and agent enrollment (≈ 4 dev-days)

**Goal**: Grants can scope to specific buckets; agents have a `default_bucket`; ACL checks at write time route correctly.

Tasks:
- `memvault-auth::grant`: introduce `GrantScope`. Custom `Deserialize` for `Grant` to back-compat-map old `TagPattern[]` scopes onto `GrantScope::Tag(...)`.
- `memvault-auth::enrollment`: add `default_bucket: Option<BucketId>`.
- `memvault-auth::verifier`: extend `AuthVerifier` to evaluate `GrantScope::Bucket` against the envelope's bucket_id.
- `memvault-api`: `bucket_grant_agent` writes `Op::BucketGrantAgent` and emits a `Signed<Grant>` with the appropriate `GrantScope::Bucket`. `put_doc`/`upload_file`/`add_entity`/`add_link` get the `bucket: Option<BucketId>` trailing param; missing param → look up caller agent's `default_bucket`.
- `plan-ai-memvault`: bucket field added to write tools; `MEMVAULT_DEFAULT_BUCKET` env var honored.
- `memctl`: `bucket grant`.
- Property tests: every old grant deserializes; new bucket-scoped grants reject access to other buckets.

Acceptance:
- Agent enrolled with default bucket "Alpha" and grants on bucket "Beta" can write to both via explicit bucket arg, defaults to Alpha. Attempts to write to "Gamma" without grant return `AuthError::NoGrant`.
- Old grants (tag patterns only) keep working unchanged on existing data.

### B3 — Bucket attach, gossip filters, and serve-side check (≈ 4 dev-days)

**Goal**: A private bucket can be flipped to attached state; cluster peers discover and start replicating. The private-bucket gossip and serve filters are in place so private data never leaks even under partition.

Tasks:
- `memvault-doc::op::BucketAttach` handling in `apply`.
- `memvault-net::gossip` **publish-side filter**: do not publish heads for envelopes whose bucket has `private_to_peer = Some(_)`. Wire this where `HEADS_TOPIC` messages are constructed.
- `memvault-net::bitswap` **serve-side filter**: the `may_serve` function from the Sync section, implemented as `memvault-net::visibility::ServeGuard` extending the existing `VisibilityFilter`. All bitswap serve paths must consult it.
- `memvault-net::gossip`: add `AdminAnnouncement::BucketCreated/Attached/Archived`.
- `memvault-api`: `bucket_attach` flips `private_to_peer` on the latest `BucketDecl` (by writing a new one with the field cleared) and triggers a heads re-publish for all CIDs in the bucket.
- `memctl bucket attach`.
- Two-peer integration test in `sim-tests/scenarios/bucket_attach.rs`: peer A keeps a bucket private through 100 writes; peer B sees nothing (verify by inspecting B's gossip log AND by direct bitswap WANT against A returning refusals); peer A runs `bucket attach`; within 5 seconds peer B has the full bucket head set and can read all entries.

Acceptance:
- Round-trip the attach across a deliberately partitioned cluster; once partition heals, peers converge on the attached state.
- Re-attaching an already-attached bucket is a no-op (idempotent).
- **Defense-in-depth probe**: a malicious peer that knows a private bucket's CID (e.g. via leak) and issues a direct bitswap WANT receives `ServeError::BucketPrivate`. Audited.

### B4 — Views become bucket-aware; the VFS gets a bucket dimension (≈ 3 dev-days)

**Goal**: Views target a bucket by default. The VFS roots can be per-bucket. Existing single-bucket-implicit views keep working.

Tasks:
- `memvault-api::types::View`: add `bucket_id: Option<BucketId>`. Old views deserialize with `bucket_id = None` (fan-out across the agent's accessible buckets).
- `memvault-api::vfs`: `ensure_root` accepts a bucket parameter and looks up a per-bucket VFS root (`(vfs:root, bucket:<bs58>)` tag pair). For backwards compatibility, `None` → original behavior (single VFS).
- `memvault-web`: views page gets a bucket selector; VFS explorer gets a bucket switcher.
- `memvault-export`: export honors bucket scoping; default is the bucket the export was launched from.

Acceptance:
- Old views and VFS work without changes. New views with `bucket_id` set hide cross-bucket entries even with permissive grants.

### B5 — Share proposal protocol (cross-cluster, no approval yet) (≈ 5 dev-days)

**Goal**: A cluster can propose a bucket share to another cluster; the receiving cluster files it in an inbox; nothing replicates yet.

Tasks:
- `memvault-auth::share`: `ShareProposal`, `ShareReply`, `BucketTrust` types with sign/verify methods analogous to existing `Grant`/`Attestation`.
- `memvault-net::share_proto`: new `/ai-memvault/share/1.0` request-response codec. Allowed on un-auth'd connections (alongside join). Per-cluster rate limit (governor crate, already in workspace).
- `memvault-store`: SHARE_INBOX, SHARE_OUTBOX tables; helpers.
- `memvault-api`: `share_propose`, `share_inbox`, `share_outbox`.
- `memctl share propose`, `memctl share inbox`, `memctl share outbox`.
- Tests: two-cluster simnet; cluster A proposes; cluster B sees inbox row; cluster A sees outbox pending; expired proposals auto-marked expired on inbox read.

Acceptance:
- 100 concurrent proposals from cluster A → cluster B's inbox is consistent, no duplicates by `proposal_id`.
- Rate-limit triggers cause `Refused` responses, not silent drops.

### B6 — Share approval, BucketTrust issuance, and federated bucket reads (≈ 6 dev-days)

**Goal**: An approver can decide a proposal; on approval, `BucketTrust` is issued; federation gossip starts flowing the bucket. Cross-cluster races resolve correctly.

Tasks:
- `memvault-api`: `share_decide`. On `Approve`, write `Signed<ShareReply>`, `Signed<BucketTrust>`; send `ShareReply` back over `/share/1.0` to the proposing cluster; gossip `FederationAnnouncement::BucketTrustEstablished` on the cluster-pair federation topic; persist in BUCKET_TRUST.
- `memvault-auth::verifier`: when serving a block at federation egress, check `BucketTrust` for the bucket+to_cluster+now in addition to existing `ClusterTrust`. This is the cross-cluster branch of `may_serve` (step 3) from the Sync section.
- `memvault-net::gossip` federation publish-side: filter `HeadAvailable` per `BucketTrust` set so we don't fan out heads to clusters that can't read them.
- `memvault-net::federation`: **pending-bucket-discovery queue.** When `BucketTrustEstablished` arrives for a bucket whose `BucketDecl` is not yet local, queue the trust as pending and retry fetching the decl via bitswap with exponential backoff (5s, 30s, 5m, then a federation-gossip request for the decl CID). Trust enters `BUCKET_TRUST` only once the decl is present.
- `memvault-api::share_decide` race handler: when persisting a `ShareReply` for a `proposal_id`, deduplicate against any existing replies. Pick the earliest by `(lamport, signer_peer_id)` per the race table; mark later replies as `superseded` for audit.
- `memvault-net::gossip` admin handler: on `Revoked(trust_cid)`, evict the trust from BUCKET_TRUST and the in-memory verifier cache within the next gossip tick.
- `memctl share approve`, `memctl share reject`.
- Two-cluster simnet test: A proposes bucket X to B with [Read]; admin agent on B approves; within 5 seconds B has the bucket heads and can read CIDs in X.
- Race-resolution tests: two-admin double-approve resolves to one canonical trust; trust-before-decl arrival eventually converges; revoke-mid-stream stops new block transfers within one gossip tick.
- Negative test: A proposes; B rejects → A's outbox shows rejected, no replication occurs, and a second proposal from A to B for the same bucket is allowed (rejection does not blacklist).

Acceptance:
- Revoking a `BucketTrust` (admin issues `Revocation` targeting the trust CID) immediately stops federation reads of that bucket; new connections from the formerly-trusted cluster get `ServeError::NoBucketTrust` for that bucket's CIDs.
- Approval can be **attenuated** at decide time (e.g. proposer asked for [Read, Write]; approver grants [Read] only and a shorter TTL).
- Pending-discovery queue drains within 5 minutes under normal network conditions.

### B7 — Share-agent MCP server and Claude Code skill (≈ 3 dev-days)

**Goal**: A focused MCP server an LLM agent runs to triage `share_inbox`; ships with a Claude skill that wires it into a daily review workflow.

Tasks:
- New crate `plan-ai-memvault-share-agent` (workspace member). Reuses `memvault-api`. 6 tools listed earlier.
- Add `.claude/skills/share-review/SKILL.md` (template below) modeled after `.claude/skills/heal-staff-pings/SKILL.md`.
- Add a server-side route (`memvault-web::api::admin::share`) so the Dioxus web UI surfaces a Share Inbox page for human review fallback.
- Wire `share_staff_ping` to the existing `mac-mgmt-healer` staff-ping pipeline so escalations show up in the existing operator workflow.

Acceptance:
- An LLM agent driving the share-agent MCP can: list pending proposals; inspect one; check the source cluster's admin-key history; check classifications present in the bucket via a `Read` action against a read-only attenuated probe-grant the proposer must pre-grant; approve, reject, or escalate.
- A human reviewer can do the same flow from the web UI without the MCP.

### B8 — Bucket quotas, GC, archive, and operations (≈ 4 dev-days)

**Goal**: Buckets are governable at runtime — per-bucket quotas, GC, archival.

Tasks:
- `memvault-api::quotas`: per-bucket caps on `bytes_stored`, `envelope_count`, `op_writes_per_minute`. Existing `governor` integration extended.
- `memvault-doc::gc`: `gc --bucket <id> --before TS` honors bucket scope; orphan attachment chunks scan the bucket-scoped manifest set.
- Bucket archive: `BucketArchive` op flips an `archived: true` flag in the bucket metadata; new writes refuse; reads continue; export still works.
- `memvault-web::ui::pages::admin`: add Buckets dashboard (list, status, quotas, archive, share buttons).
- Sim-test scenarios: bucket fills its quota → writes start failing with `QuotaExceeded`, audit shows the refusals. Archived bucket survives a daemon restart in archived state.

Acceptance:
- `/chaos-test 10` rounds with bucket creates / attaches / shares interleaved with daemon restarts shows no inconsistency.
- A 1 GB bucket can be GC'd (manual `memctl gc --bucket <id> --before <ts>`) without affecting other buckets in the same cluster.

**Total**: ~35 dev-days ≈ 7 calendar weeks for a 2-engineer team (one focused on the data plane B1–B4, the other on the share protocol B5–B7). The extra two days vs. the earlier draft cover the gossip + serve-side filter wiring in B3 and the race-resolution handlers in B6 — see the Sync section for why both are load-bearing.

---

## Claude Code working agreement additions

Append to existing `CLAUDE.md`:

```markdown
## Buckets

- Every write API now takes an optional `bucket: Option<BucketId>`. When refactoring older code paths, do not silently pick the cluster default — propagate `None` and let `LocalClient` resolve to the calling agent's `default_bucket`.
- The classification linter is the single source of truth for bucket-tag well-formedness. Any new code path that builds a Signed<T> must call `lint_tags` before signing. The CI `cargo test -p memvault-core --lib lint_tags` runs the property tests.
- New CRDT Op variants must be appended (never inserted in the middle of the enum) to keep DAG-CBOR serialization stable. Mark with `// added B1, removable never` comments.
- The BY_BUCKET index is **derived**. Repair-index rebuilds it from BLOCKS by reading the `bucket:` tag from each envelope. Never edit BY_BUCKET directly except in `insert_envelope` and the repair-index path.
- Old grants without a bucket scope keep working forever. New grants without a bucket scope are an error in code review — every new grant must either pin a bucket or be `GrantScope::Any` with a comment explaining why.

## Sync and serve checks

- The serve-side check (`memvault-net::visibility::ServeGuard::may_serve`) is the security boundary, not the gossip filter. Every new bitswap serve path MUST consult it. Gossip filtering is a performance optimization only.
- Never weaken the four checks in `may_serve` (visibility, private-bucket, cross-cluster bucket-trust, revocation) by adding early-returns or skip-paths. New conditions go *into* the function, not around it.
- All cross-cluster trust evaluations are time-bound. Tests must construct trusts with explicit `not_after_ns` and verify expiry takes effect at serve time (not just at insert).
- The pending-bucket-discovery queue (B6) is the only place trust storage is allowed to be eventually-consistent with bucket-decl storage. Anywhere else, treat trust-without-decl as a programming error.
- Lamport is authoritative for ordering. `wall_ns` is for UX only; never use it as a tiebreaker in correctness paths.

## Share protocol

- `/ai-memvault/share/1.0` is the second protocol allowed on un-authenticated connections (alongside `/join/1.0`). Any change to its codec must keep the version byte at the front and preserve forward-compat (`#[serde(default)]` on new fields).
- A first-contact share proposal (no prior `ClusterTrust`) MUST be tagged `provisional_first_contact = true` in the inbox row so reviewers see it. Hardcoded auto-approvers in tests are forbidden outside `sim-tests/scenarios/share_*`.
- `BucketTrust` is the only thing that opens federation reads on a per-bucket basis. Do not add side-channels (e.g. config-file allowlists) that bypass it.
- Double-decide races are resolved deterministically by `(lamport, signer_peer_id)`. Do not introduce wall-clock tiebreaks.
```

---

## Sub-agents to register in `.claude/skills/`

Following the existing conventions (frontmatter, allowed-tools, argument-hint, etc.), add:

### `.claude/skills/bucket-cutover/SKILL.md`

```markdown
---
name: bucket-cutover
description: Use when migrating a codebase area from implicit-default-bucket to explicit-bucket-aware. Walks every call site, propagates the bucket param, updates tests, and adds compatibility shims.
allowed-tools: Read Edit Write Grep Glob Bash(cargo:*)
---

# Bucket Cutover Skill

You are migrating one crate area at a time from the pre-B2 API to the bucket-aware API. Work in this order:

1. `cargo check -p <crate>` to baseline.
2. `grep -rn "put_doc\|upload_file\|add_entity\|add_link" src/` in the target crate to find call sites.
3. For each call site, decide:
   - Test/example code → pass `None` (default bucket).
   - User-facing handler → take a bucket from request/env/config.
   - Internal pipeline → propagate from caller (do not invent a bucket).
4. Update tests to construct an explicit bucket where the test's intent is bucket isolation.
5. Run `cargo test -p <crate>`.
6. If you touched `memvault-api` or `plan-ai-memvault`, also run `/chaos-test 5`.

Refuse to commit if you've added a hard-coded `BucketId::default()` anywhere other than the cluster genesis path.
```

### `.claude/skills/share-review/SKILL.md`

```markdown
---
name: share-review
description: Use when asked to triage pending memvault cross-cluster share proposals via the share-agent MCP. Approves, attenuates, rejects, or escalates each.
allowed-tools: mcp__share__share_inbox_list mcp__share__share_inspect mcp__share__share_check_classifications mcp__share__share_approve mcp__share__share_reject mcp__share__share_staff_ping
---

# Share Review Skill

You are reviewing pending memvault share proposals.

## Step 1: Triage
Run `share_inbox_list` to list pending proposals. For each:
- If `provisional_first_contact = true`, ALWAYS run `share_inspect` and verify the proposer's cluster admin-key signature chain. If unverifiable, `share_reject` with reason "first-contact identity not verifiable".
- If the proposer cluster is in the local `ClusterTrust` set, proceed to step 2.

## Step 2: Classification check
Run `share_check_classifications` on the bucket. If any envelope is `classification:confidential`, escalate via `share_staff_ping`. Confidential data never auto-approves.

## Step 3: Decide
- `internal` data + trusted cluster + proposed [Read] only → `share_approve` with attenuation `ttl=7d`.
- `internal` data + proposed [Write] → `share_approve` only if the requesting agent is in the receiving cluster's `agent_allowlist`; otherwise reject.
- `public` data → approve as proposed.
- Any uncertainty → `share_staff_ping` with a one-line summary and link.

Do not approve more than 10 proposals without surfacing a summary. Bias toward rejection when classification or trust is unclear.
```

---

## What to actually do this week

The plan is concrete enough to execute. The first sprint — **B1** — is the only one that's load-bearing on the rest; the others can ship in any order after it. Concrete first-week tasks:

1. **Day 1**: Land the `BucketId` type and the `bucket:` linter rule in `memvault-core`. PR small, easy to review, no behavior change yet. Run `cargo test -p memvault-core`.
2. **Day 2**: Add the three new redb tables and the `EnvelopeMeta.bucket_id` field in `memvault-store`. Extend `insert_envelope` and `reindex_block`. Add `query_by_bucket`. Tests in `memvault-store/tests/`.
3. **Day 3**: Add `BucketDecl` payload and bucket Op variants in `memvault-doc`. Apply functions for each.
4. **Day 4**: Genesis path writes the default `BucketDecl`. `LocalClient.bucket_create`, `bucket_list`, `bucket_get`. `memctl bucket new|list|show`.
5. **Day 5**: Integration test in `memvault/tests/integration/buckets_b1.rs` covering: create, list, default-on-genesis, private-to-peer isolation, BY_BUCKET query correctness, BY_TAG/BY_BUCKET coherence.

After B1 lands, **B2** (grants) is unblocked and parallel work on **B5** (share protocol) can begin against the still-default-bucket world; they reconnect at **B6**.

---

## What this plan deliberately does NOT do

- **No vector embeddings.** The existing tantivy + graph traversal works. Vectors are a v8 question.
- **No envelope schema bump.** Buckets ride on the existing tag set + EnvelopeMeta. The version byte stays at 1.
- **No replacement of Views.** Views remain a read-only saved-filter primitive, now bucket-aware. They are *complementary* to buckets, not redundant.
- **No new ALPN beyond `/ai-memvault/share/1.0`.** Federation, heads gossip, bitswap all reuse the existing protocols, just with bucket filtering.
- **No auto-approval policy in the share-agent.** Every decision is recorded; the *only* shortcut is `share_approve` issued by an authorized principal (human or LLM-with-grants). Audit is the universal backstop.
- **No tag-replacement migration.** Existing `compartment:`, `agent:`, `topic:` tags continue to work as classifications/discriminators. Bucket is added next to them, not in place of them.
- **No linearizability or cross-bucket transactions.** memvault offers strong eventual consistency on doc state (Loro CRDT), causal consistency on op ordering, and an append-only audit log. That's the contract — see the Sync section for the full statement. Code that needs "write-then-immediately-read-on-another-peer" must poll or subscribe.
- **No global ordering across clusters.** Lamport clocks are per-cluster. Cross-cluster ordering uses `wall_ns` for display only.
- **No automatic recovery from BucketTrust revocation.** When a trust is revoked mid-replication, the receiving cluster may have a partial bucket. Detecting and surfacing partial state is a B8 metric, not an automatic re-request.

---

## Caveats

- **Adding `bucket` to write trait methods is a wide refactor** (call sites in `memctl`, `plan-ai-memvault`, `memvault-web`, `memvault-export`, `memvault-import`, tests). Budget a day of pure mechanical updates after B1 lands.
- **`BucketTrust` is per-pair** (from_cluster → to_cluster, per bucket). If cluster A wants to share bucket X with three different clusters, that's three trusts. This is intentional (per-pair attenuation), but the web UI should make the multi-target flow ergonomic in B7.
- **Old envelopes** (v1 without a bucket tag) land in the default bucket on first indexing. `memctl repair-index` re-derives BY_BUCKET correctly. If a cluster has years of data and you want some of it to live in a non-default bucket, that requires a one-time `memctl bucket reassign --from default --to <id> --filter <tag>` operation (a new helper; out of scope of B1 but trivial to add).
- **The share-agent is a privileged role**. Its keys should rotate on the existing `AgentKeyRotation` cadence; its grants should be the minimal set needed (Action::Admin scoped to share inbox/outbox plus Read on the buckets it triages for classification checks). Document this in the share-agent crate's README.
- **First-contact identity verification** (B5) assumes the receiver knows the proposer's cluster admin key out-of-band (e.g. via a `ClusterTrust` row or a CLI flag). For pure first-contact, the inbox row stores the proposal but the reviewer must verify the proposer key via a side channel. Phase 8 of the v6 plan (multi-sig / TOFU policies) can layer better solutions on top later.
- **Firewall asymmetry on `/share/1.0`**. The reply leg requires the receiving cluster to be able to dial out to the proposing cluster. If both clusters are inbound-only behind NAT, the reply gets stuck. The relay subsystem (`mac-mgmt-relay`) already exists for this exact problem in the daemon; piggybacking shared-cluster discovery on it is a v8 conversation, not a B6 blocker.
- **Partial replicas on revocation.** When a `BucketTrust` is revoked while a federated peer is mid-fetch, the receiving cluster keeps the blocks it already fetched (per the audit-log monotonicity guarantee — we don't unmake writes). The bucket appears "complete-ish" until the operator notices via the B8 completeness metric. If this matters for compliance, operators must explicitly run `memctl bucket purge --bucket <id> --from-cluster <id>` after a revocation; this is an opt-in destructive op, not automatic.
- **Pending-discovery queue is bounded.** If a `BucketTrust` references a `BucketDecl` that never arrives (e.g. proposer cluster goes offline permanently), the trust stays in the pending queue until an operator clears it. The queue has a hard cap (default 1000 pending) to prevent unbounded growth.
- **Lamport rollover.** `u64` Lamport at 1M ops/sec rolls over in ~585 thousand years. Not a real concern, but tests should not rely on Lamport being monotonic across `wall_ns`.
