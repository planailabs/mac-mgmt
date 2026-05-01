# AI Memvault — Attachments & MCP Plan

A focused plan for adding file/attachment handling to memvault, including the changes the MCP server needs to expose attachments to openclaw and other agents.

This is a sibling document to `plan-ai-memvault.md` (the main build plan, currently v6). It reuses that plan's commitments — IPLD/DAG-CBOR data model, libp2p transport, beetswap, redb, classification tags, signed envelopes — and adds attachments as a new payload kind alongside markdown documents and knowledge-graph entities.

---

## Table of contents

1. Decisions assumed (from elicitation)
2. What attachments are and aren't
3. Data shapes
4. Storage model: chunking and CIDs
5. Replication policy: hybrid eager + lazy
6. Content type and metadata
7. Extraction and indexing
8. PII scanning and redaction for attachments
9. Egress and classification
10. Network protocols
11. MCP server: resources, tools, and changes
12. Workspace changes
13. Cargo dependencies (additions)
14. Phase plan
15. Open decisions
16. Risks
17. First-week tasks

---

## 1. Decisions assumed

These came from the elicitation step and shape every section below:

1. **Size profile: mixed.** Design for ≤10 MB, ≤100 MB, and ≤1 GB tiers in one system. Not three systems.
2. **Replication: hybrid.** Small files (≤10 MB) eager-replicated cluster-wide; larger files lazy via beetswap. Threshold configurable.
3. **Content type: client-supplied MIME only.** No server-side sniffing in v1. Trust the writer.
4. **Extraction depth: extract + tantivy index.** Extract text from common formats (PDF, DOCX, TXT, MD, HTML), index in tantivy. No vector embeddings yet.
5. **PII handling: scan + redact.** Run the existing memvault PII detector on extracted text; produce redacted derivative attachments where required.
6. **Large-file transfer: UnixFS-style chunking + DAG-PB.** IPFS-compatible file representation. Tools to import/export with kubo work for free.
7. **MCP: prominent.** Attachments are MCP `resources`. Tools for `attach_file`, `redact_attachment`, etc. mirror the API.

---

## 2. What attachments are and aren't

An attachment is **an opaque (or extracted) file with metadata**, addressable by CID, signed by the writer, classified, and replicable across the cluster. Attachments are a third payload kind alongside `Document` (markdown+CRDT) and `Entity` (graph node).

**Attachments are not:**

- Mutable. A "modified" attachment is a new attachment with new content, optionally linked via `provenance` to its predecessor. CRDT semantics do not apply to attachment bytes.
- Streamed for write. Writes are atomic over the full content. Reads can be ranged (via UnixFS chunk traversal) but writes are commit-then-publish.
- Replacements for documents. If a memory is fundamentally a markdown note with an inline image, the note is a `Document` and the image is an `Attachment` referenced from frontmatter or via a new tag scope.

**Attachments compose with existing memvault primitives:**

- They are wrapped in `Signed<AttachmentManifest>` envelopes — same audit, signing, retraction, history machinery as anything else.
- They carry `classification:<level>` and `handling:<flag>` tags. Egress policy applies to them like everything else.
- They participate in `provenance` ancestry — a redacted attachment's manifest references the source attachment's CID; an attachment extracted from a parent document has the document as its provenance parent.
- They appear in the audit log as `op_kind: AttachmentCreated`, `AttachmentExtracted`, `AttachmentRedacted`.

---

## 3. Data shapes

Three new IPLD types: `AttachmentManifest`, `AttachmentChunkRef`, and `ExtractedText`. Plus `EgressDestination` semantics extend cleanly.

### 3.1 AttachmentManifest

The metadata envelope, signed and indexed. Bytes live elsewhere (see §4).

```rust
struct AttachmentManifest {
    // Content addressing
    content_root:    Cid,            // root of the UnixFS DAG (or the raw block for tiny files)
    content_size:    u64,            // total bytes
    chunk_layout:    ChunkLayout,    // describes how content_root resolves to bytes

    // Identity and naming
    filename:        Option<String>, // original filename (UTF-8); not authoritative
    mime_type:       String,         // client-supplied; "application/octet-stream" if unknown

    // Optional fast-path metadata
    sha256:          Option<[u8; 32]>,   // for non-CID interop (cloud APIs etc.)
    width_height:    Option<(u32, u32)>, // images
    duration_ms:     Option<u64>,        // audio/video

    // Extraction/derivation links (filled in over time, NOT in initial write)
    extracted_text:  Option<Cid>,        // Signed<ExtractedText> if extraction has happened
    derived_from:    Option<Cid>,        // for redacted/transcoded derivatives
    pii_findings:    Option<Cid>,        // Signed<PiiFindingsBlock> if scanned

    // Replication hint
    replication:     ReplicationHint,
}

enum ChunkLayout {
    /// Small files: content_root is a single raw IPLD block of bytes (≤ inline_threshold).
    Inline { size: u32 },
    /// Medium and large files: UnixFS file DAG rooted at content_root.
    /// Standard IPFS chunking (default 256 KiB chunks, balanced layout).
    UnixFs { chunk_size: u32, layout: UnixFsLayout },
}

enum UnixFsLayout { Balanced, Trickle }

enum ReplicationHint {
    Eager,                                   // pre-fetch on every peer
    Lazy,                                    // fetch on demand
    PinnedBy(Vec<PeerId>),                  // explicitly pinned by these peers
}
```

The `Signed<AttachmentManifest>` envelope is just like any other memvault envelope: classification tag required, visibility set, audit/retraction/history apply.

The manifest is **always small** (<1 KiB) regardless of the underlying file size. The expensive bytes live behind `content_root`.

### 3.2 ExtractedText

Produced by the extraction pipeline (§7). A separate signed envelope so it can be re-extracted, retracted, or revised independently of the source.

```rust
struct ExtractedText {
    source:           Cid,           // attachment manifest CID
    extractor:        String,        // "pdf-extract@0.7", "docx-rs@0.5", "raw-utf8"
    extractor_version: String,
    extracted_at_ns:  u64,
    text:             String,        // full extracted plain text
    page_breaks:      Vec<u32>,      // byte offsets where pages/sections break
    warnings:         Vec<String>,   // extraction warnings (e.g. "OCR not run")
}
```

`Signed<ExtractedText>` is itself a memvault envelope with `kind:extracted-text` tag and `provenance: [source]`. It inherits the source's classification.

### 3.3 PII findings on attachments

Already covered by the existing `PiiFinding` type. We add a wrapper:

```rust
struct PiiFindingsBlock {
    target:        Cid,              // attachment manifest CID
    scanned_text:  Cid,              // ExtractedText CID that was scanned
    findings:      Vec<PiiFinding>,
    detector:      String,
    scanned_at_ns: u64,
}
```

`Signed<PiiFindingsBlock>` is its own envelope, classified at least as high as the source (it contains the sensitive locations).

### 3.4 Tag conventions for attachments

| Tag | Meaning |
|---|---|
| `kind:attachment` | required on `AttachmentManifest` envelopes |
| `kind:extracted-text` | required on `ExtractedText` envelopes |
| `kind:redacted-attachment` | redacted derivative attachments |
| `mime:image/png` (etc.) | optional; mirror of manifest's `mime_type` for index queries |
| `attachment-of:<doc_id>` | optional back-reference if the attachment belongs to a specific document |

The classification linter rules from the main plan apply unchanged.

---

## 4. Storage model: chunking and CIDs

Two storage paths share one IPLD addressing scheme.

### 4.1 The inline-vs-chunked threshold

Files ≤ `inline_threshold` (default **64 KiB**) are stored as a single raw block. The block's CID *is* `content_root`. `ChunkLayout::Inline { size }`.

Files > 64 KiB are chunked using UnixFS:

- Default chunk size: **256 KiB** (matches kubo defaults).
- Default layout: balanced tree.
- `content_root` is the CID of the root DAG-PB node. Walking it yields the full file.

The `inline_threshold` is small on purpose: most chunkable formats benefit from range-fetch and dedup, and 64 KiB is comfortably below the bitswap message limit.

### 4.2 Why UnixFS

Three reasons:

1. **IPFS interop.** A memvault attachment is a kubo-readable file. Operators can mount via `ipfs get` or push to public gateways without our cooperation. This is operationally valuable even for a closed cluster.
2. **Range fetch.** UnixFS chunks are 256 KiB; `read_range(cid, 1_000_000, 1_005_000)` walks only the chunks covering that byte range. Critical for large PDF and video previews.
3. **Maintained crates exist.** `ipld-dagpb` (codec) and `rust-unixfs` (file DAG construction and walking) are current. We don't need to invent the format.

### 4.3 Where bytes live

Chunks (and inline blocks) are stored in the same redb `blocks` table the rest of memvault uses. They're just block bytes keyed by CID. The blockstore impl doesn't care if a block is part of an envelope, an op, or a UnixFS chunk.

This is the right call: it means beetswap, replication, retention, and GC all work on attachment chunks for free.

What changes:

- A new redb table `attachment_chunks` mapping `manifest_cid → Vec<chunk_cid>` for fast "does this peer have all the chunks for X?" queries. This is a derived index, rebuildable.
- A new redb table `pinned` mapping `manifest_cid → PinReason` so eager-replicated and explicitly-pinned attachments survive future GC passes.

### 4.4 Eager-replicated attachments

When `ReplicationHint::Eager` is set (or auto-set because `content_size ≤ eager_threshold`, default **10 MB**):

1. Writer writes the manifest envelope and all chunk blocks normally.
2. The manifest is gossiped via the heads topic (it's a write like any other).
3. Receiving peers, on observing a new `Signed<AttachmentManifest>` with `Eager`, immediately fetch the chunks via beetswap and pin them locally.
4. Pin entries are recorded in the `pinned` table with `PinReason::EagerByPolicy`.

Peers may opt out of eager replication via config (`eager_attachment_max_size_mb = 0` disables it entirely).

### 4.5 Lazy attachments

`ReplicationHint::Lazy` (or auto-set because content is large): manifest is gossiped, chunks are not. A reader fetches chunks on first access. Once fetched, chunks are cached in the local blockstore subject to a size-bounded LRU (separate from the main blockstore retention; large attachments shouldn't evict envelopes).

```rust
struct AttachmentCache {
    max_bytes:  u64,                 // default 5 GiB
    eviction:   EvictionPolicy,      // LRU by access time
}
```

The cache is operator-tunable. A peer that wants to permanently keep a lazy attachment runs `memctl pin <manifest_cid>` which adds a `PinReason::Manual` entry.

### 4.6 Pin lifecycle

Pinning prevents attachment chunks from being evicted by the LRU OR garbage-collected by the manual `memctl gc` command:

```rust
enum PinReason {
    EagerByPolicy,                   // auto-pinned because manifest had Eager hint
    Manual,                          // operator pinned via memctl
    PinnedByGrant(Cid),              // future: a grant requires retention
}
```

Unpinning is explicit. The webapp's `AdminPanel` (Phase 7 of the main plan, extended in Phase 3 of this plan) lists pinned attachments by reason.

### 4.7 What about a 1 GB file?

Worked example: a 1 GiB video uploaded with `Lazy` hint:

1. Writer chunks the file at 256 KiB → 4096 chunks. Builds a balanced UnixFS tree (`rust-unixfs::file::adder`). Each chunk is a DAG-PB block; intermediate nodes are also DAG-PB blocks pointing at child CIDs. The root is `content_root`.
2. Writer writes all those blocks atomically into its own blockstore in a single redb transaction.
3. Writer publishes `Signed<AttachmentManifest>` with the root CID and `chunk_layout: UnixFs { chunk_size: 256*1024, layout: Balanced }`.
4. Manifest gossips on heads topic. Other peers see the manifest and don't fetch.
5. Reader requests `read_range(manifest_cid, 0, 1_000_000)` — the manifest tells them the chunk_size and layout; they walk the UnixFS tree, requesting only the chunks covering the first ~1 MB via beetswap. Each chunk arrives, is verified, is cached.
6. Reader's `AttachmentCache` may evict these chunks later if it fills.

This is the path the existing IPFS ecosystem already optimizes. We're not inventing anything.

---

## 5. Replication policy: hybrid eager + lazy

### 5.1 Auto-classification by size

```rust
fn default_replication_hint(size: u64, classification: ClassificationLevel) -> ReplicationHint {
    if size <= EAGER_THRESHOLD_BYTES /* default 10 MiB */ {
        ReplicationHint::Eager
    } else {
        ReplicationHint::Lazy
    }
}
```

The writer can override (e.g., to force `Eager` for a 50 MB file the team will all want, or `Lazy` for a 1 MB file that's noisy and only one peer needs).

### 5.2 Per-tag-scope policy (Phase 3)

Operators can declare policies that override per-attachment hints:

```toml
[[attachment_pin_policy]]
match_tag = "classification:public"
strategy = "eager"

[[attachment_pin_policy]]
match_tag = "kind:redacted-attachment"
strategy = "eager"

[[attachment_pin_policy]]
match_tag = "compartment:archive-only"
strategy = "lazy"
```

Phase 1 ships with auto-classification only. Phase 3 adds the policy file.

### 5.3 What "eager" actually does on the wire

Nothing special at the bitswap layer. When a peer observes a `Signed<AttachmentManifest>` with `Eager`:

1. It enqueues a fetch of `content_root`.
2. As blocks arrive, it walks the UnixFS tree and enqueues children.
3. Each chunk is stored in `blocks` and noted in `attachment_chunks`.
4. A `pinned` entry is created with `PinReason::EagerByPolicy`.

This is just normal beetswap traffic — no separate "eager" protocol. The hint is advisory; peers may decline (`eager_attachment_max_size_mb = 0` config).

### 5.4 Heartbeat: who has the chunks?

For replication observability, peers periodically gossip a small Bloom filter of which manifests they fully have, on `ai-memvault/attachments/v1`. This lets admins answer "which peers don't have manifest X yet?" without polling. It's purely diagnostic — not consulted for serve decisions.

---

## 6. Content type and metadata

### 6.1 Trust-the-client (with caveats)

The writer supplies `mime_type`. The store does not sniff. Rationale:

- Sniffing requires a dependency (`infer`, `tree_magic_mini`) and gets it wrong on edge cases.
- The writer typically already knows (uploaded from a filesystem with extensions, downloaded from a URL with `Content-Type`).
- Wrong MIME types are an audit/review concern, not a security one — egress policy and PII scanning don't trust MIME for security decisions.

The webapp shows both the declared MIME and a "this looks like X" hint computed lazily on first preview, but the canonical metadata is what the writer wrote.

### 6.2 What "trust" doesn't mean

It does NOT mean we use the MIME type to decide whether to extract or scan. The extractor dispatches on **declared MIME** but if extraction fails (it's not actually a PDF, just labeled as one), we record the failure and proceed without extracted text. PII scanning then has nothing to scan; the attachment passes through with no PII findings — and that's an accurate reflection of "we couldn't see inside it."

### 6.3 Filename safety

Filenames in `AttachmentManifest::filename` are advisory only. They are NEVER:

- used as filesystem paths
- used as URL components
- used to guess content type
- normalized in lossy ways

Display layers (webapp, MCP) sanitize for rendering but the canonical value is preserved.

### 6.4 Hash interop

`sha256` is optional and supplied by the writer if known. It exists for interop with cloud APIs and content-addressed systems that aren't IPFS. The CID's BLAKE3 multihash is the canonical addressing; sha256 is metadata.

---

## 7. Extraction and indexing

A new crate `memvault-extract` runs at write time when an attachment manifest is committed.

### 7.1 Extraction pipeline

```rust
trait Extractor: Send + Sync {
    fn supports(&self, mime: &str) -> bool;
    async fn extract(&self, content: &[u8], hints: &ExtractionHints) -> Result<ExtractedText>;
}

struct ExtractionRegistry {
    extractors: Vec<Box<dyn Extractor>>,
}
```

Default extractors:

| MIME | Crate | Notes |
|---|---|---|
| `text/plain` | (none) | UTF-8 decode; replace invalid sequences |
| `text/markdown` | `pulldown-cmark` (already in plan) | strip formatting |
| `text/html` | `scraper` or `html2text` | strip tags |
| `application/pdf` | `pdf-extract` | text-only; OCR is Phase 5 |
| `application/vnd.openxmlformats-officedocument.wordprocessingml.document` | `docx-rs` | DOCX |
| `text/csv`, `application/json` | (none) | UTF-8 decode |

Anything else gets no extracted text. The pipeline records the attempt with a "no extractor for MIME X" warning and moves on.

### 7.2 When extraction runs

By default, **synchronously at write time**, in a Tokio blocking-pool task. Reasons:

- The writer already has the bytes resident.
- Synchronous extraction means tantivy index, PII scan, and egress decisions can rely on extracted text being available immediately.
- For very large files, extraction is configurable to be deferred — `extract_async_threshold_mb` (default 50) sends extraction to a background worker queue. The manifest is committed without `extracted_text` set; extraction completes later and writes a new `Signed<ExtractedText>` envelope, updating the manifest's `extracted_text` field via a `ManifestUpdate` block (next subsection).

### 7.3 Manifest updates

Manifests are immutable in CID terms (any change produces a new CID). To express "the extracted text for manifest X is now available," we use a small mutable-pointer pattern:

```rust
struct ManifestUpdate {
    target_manifest:  Cid,           // original manifest
    extracted_text:   Option<Cid>,
    pii_findings:     Option<Cid>,
    derived_from:     Option<Cid>,
    updated_at_ns:    u64,
}
```

`Signed<ManifestUpdate>` is its own envelope with `provenance: [target_manifest]`. The query layer materializes "current manifest state" by folding the original manifest with all its `ManifestUpdate` envelopes (newest wins per field).

This is intentionally CRDT-shaped without using a full Loro container — manifest updates are sparse and infrequent, and last-writer-wins per field is fine.

### 7.4 Tantivy indexing

`memvault-query::index` already indexes markdown body text. We extend the schema:

```rust
// schema fields (additive; v2 of the schema)
attachment_manifest_cid: STRING | STORED
attachment_filename:     TEXT | STORED
attachment_mime:         STRING
attachment_text:         TEXT                  // extracted text, fed to default analyzer
attachment_size:         I64 | STORED | FAST
```

This is a v2 schema. The migration runner (already planned) adds the new fields and re-indexes existing attachments by walking the `by_tag` index for `kind:extracted-text` envelopes.

### 7.5 Search semantics

A normal `query` call returns mixed results: documents and attachments interleaved by relevance. Each `Hit` carries a `kind: HitKind` so consumers can filter:

```rust
enum HitKind {
    Document(DocId),
    Attachment(Cid),                 // manifest CID
    Entity(EntityId),
}
```

The webapp's `MemoryList` already renders heterogeneous lists; attachments get their own row template (icon by MIME, size, classification badge).

---

## 8. PII scanning and redaction for attachments

### 8.1 Scanning

PII scanning runs when `ExtractedText` becomes available — either inline at write or asynchronously after deferred extraction. The detector is the existing `memvault-policy::pii::detector` (regex + Aho-Corasick).

```rust
// in memvault-policy::pii (extended)
async fn scan_attachment(manifest_cid: Cid) -> Result<PiiFindingsBlock> {
    let extracted = load_extracted_text(manifest_cid).await?;
    let findings = detector.scan(&ScanInput::PlainText(&extracted.text)).await?;
    Ok(PiiFindingsBlock {
        target: manifest_cid,
        scanned_text: extracted.cid,
        findings,
        detector: detector.id(),
        scanned_at_ns: now_ns(),
    })
}
```

The `Signed<PiiFindingsBlock>` is written, and a `ManifestUpdate` is published referencing it.

If extraction failed (binary, unsupported, encrypted), no PII scan runs. The attachment is treated as opaque — the egress policy then has to either reject it outright (for destinations that forbid `handling:pii` and where PII status is unknown) or accept the risk.

### 8.2 Redaction

For text-extractable attachments, redaction produces a new attachment whose content is the cleaned-up extracted text re-rendered into a plain format.

```rust
async fn redact_attachment(
    manifest_cid: Cid,
    policy: RedactionPolicy,
) -> Result<RedactionResult> {
    let extracted = load_extracted_text(manifest_cid).await?;
    let cleaned = cleaner.apply(&extracted.text, &policy, &findings).await?;

    // Materialize the cleaned text as a new attachment.
    let new_attachment = put_attachment(
        bytes:        cleaned.as_bytes(),
        mime:         "text/plain",                  // redacted output is plain text
        filename:     Some(format!("{}.redacted.txt", original_filename)),
        tags:         vec![tag("kind", "redacted-attachment"), ...],
        provenance:   vec![manifest_cid],
        visibility:   /* same as source or one level lower */,
    ).await?;

    Ok(RedactionResult {
        output_cid: new_attachment,
        report_cid: pii_findings_block_cid,
        findings,
        applied: cleaned.applied,
    })
}
```

Redaction does **not** modify the original PDF/DOCX bytes. We do not attempt to re-render PDFs with redacted regions in v1 — that's a hard problem (font subsetting, layout reflow, image redaction) and the available crates aren't ready for production. The redacted output is plain text. If a destination needs a redacted PDF, the operator runs an external tool against the redacted text.

This is a deliberate scope cut. Phase 4 may revisit with `pdf-writer` if the use case warrants it.

### 8.3 Image and binary attachments

Images and other binary formats have no text to scan. They get no `ExtractedText`, no `PiiFindingsBlock`. Their classification and `handling:` tags are the only signal. Egress policy treats unknown content conservatively (see §9).

OCR is explicitly Phase 5+ if at all. Adding `tesseract` is a significant native-dependency commitment.

---

## 9. Egress and classification

### 9.1 Egress policy on attachments

The existing `check_egress(cid, dest)` in the main plan extends naturally:

1. Load envelope (the attachment manifest).
2. Extract classification, handling, compartments from envelope tags.
3. Apply destination policy.
4. **If extraction failed AND `handling:pii` is not set AND destination forbids unmarked PII:** return `Deny { reasons: ["unscannable content; cannot rule out PII"] }`. Operators can override by explicitly tagging `handling:no-pii` (asserting they have manually verified).
5. Otherwise standard policy evaluation.

### 9.2 Egress and chunked content

For lazy attachments, the receiving peer might not have all chunks locally when egress is requested. Options:

1. **Block on fetch.** `check_egress` triggers chunk fetch via beetswap, waits, then evaluates. Slow for large files.
2. **Refuse if not fully local.** Return `EgressDecision::Deny { reasons: ["attachment not fully fetched locally; pin first"] }`.

We default to **(2) refuse**. Eager-replicated and pinned attachments pass through immediately; lazy attachments must be explicitly pinned before egress. This makes the operational model honest: "if you want to send it, you have to have it." The webapp's `EgressGate` shows a "Pin and retry" button if this is the failure mode.

### 9.3 The send path

Egress happens in the agent code, not in memvault itself — memvault answers "may you?" but doesn't perform the send. For attachments specifically, the agent reads chunks via `MemvaultClient::read_attachment_range` and streams them to the destination. Agents that need to call cloud LLMs typically:

1. `check_egress(manifest_cid, dest)` → `Allow` or `AllowWithRedaction`.
2. If `AllowWithRedaction`: call `redact(...)` → get new manifest CID → re-check egress on the redacted CID.
3. Read the (possibly redacted) attachment content via streaming reads.
4. Send to destination.
5. Audit-log the egress (memvault writes the `EgressPerformed` envelope on the agent's behalf when reads are correlated with an egress check).

---

## 10. Network protocols

No new ALPNs. Attachment storage and replication piggyback on existing memvault protocols.

### 10.1 Block exchange

UnixFS chunks and inline attachment blocks travel over the existing `beetswap::Behaviour`. Same auth gating, same visibility check at serve time. Visibility is enforced on the *manifest envelope* — chunks themselves are unsigned raw IPLD blocks and are served to any authenticated peer who knows their CID. This is fine because:

- Chunk CIDs are only learned via the manifest, which is visibility-filtered at gossip time.
- Even if a chunk CID leaks, the chunk is meaningless without the surrounding tree structure (and the tree root is in the manifest, which is gated).

### 10.2 Heads gossip extension

The existing `ai-memvault/heads/v1` topic carries `DocumentHead` and (with this plan) `AttachmentManifest` head announcements. New `HeadKind` enum:

```rust
enum HeadKind {
    Document(DocumentHead),
    Attachment(AttachmentHeadRef),
}

struct AttachmentHeadRef {
    manifest_cid:   Cid,
    classification: ClassificationLevel,
    visibility:     Visibility,
    replication:    ReplicationHint,
    size:           u64,
}
```

The visibility-aware gossip filter remains the only access control on discovery.

### 10.3 Attachment heartbeat — `ai-memvault/attachments/v1`

A new low-volume topic. Each peer publishes a Bloom-filtered summary of fully-replicated manifests every 5 minutes (configurable). Used for diagnostics; not for serve decisions.

```rust
struct AttachmentHeartbeat {
    peer:        PeerId,
    sent_ns:     u64,
    bloom:       Vec<u8>,            // Bloom filter of manifest CIDs we have all chunks for
    bloom_seed:  u64,
    bloom_bits:  u32,
}
```

Phase 8 deliverable; not required for Phase 1.

### 10.4 Federation

Federation peers fetch attachments the same way as intra-cluster: manifest discovery via gossip, chunk fetch via beetswap, all subject to the same visibility/grant/classification checks. No additional federation work specific to attachments.

One nuance: a federated `Eager` attachment is **not** auto-pinned by federation peers. They receive the manifest gossip but treat it as `Lazy` regardless of the hint, because eager-pinning across federation could grow storage uncontrollably. Federation peers fetch on demand and cache per their LRU.

---

## 11. MCP server: resources, tools, and changes

### 11.1 What MCP gives us for attachments

MCP's three primitives map cleanly:

- **Resources** are read-only context the model can pull in. Attachments are perfect resources: each has a stable URI (`memvault://attachment/<manifest_cid>`), a MIME type, optional size, and content readable on demand.
- **Tools** are actions the model can invoke. We expose tools for write operations: `attach_file`, `redact_attachment`, `pin_attachment`.
- **Prompts** we don't use for attachments specifically.

The MCP 2026 spec supports streaming for large resources, OAuth 2.1 for auth, and binary content via base64 or resource URI redirection. Both patterns we'll use.

### 11.2 Resource URIs

```
memvault://attachment/<manifest_cid>                  → the attachment itself
memvault://attachment/<manifest_cid>/text             → extracted text (if available)
memvault://attachment/<manifest_cid>/metadata         → manifest as JSON (no bytes)
memvault://attachment/<manifest_cid>/findings         → PII findings (if scanned and authorized)

memvault://document/<doc_id>                          → markdown document
memvault://document/<doc_id>/attachments              → list of AttachmentHeadRefs
memvault://entity/<entity_id>                         → graph entity
memvault://search?q=...                               → search results
memvault://audit?author=...&from=...                  → audit query
```

The existing memvault MCP server (out of scope of the main plan but assumed to exist for openclaw integration) gains these URI schemes. The pattern is consistent with MCP's evolution toward stable URIs as the primary way to identify content.

### 11.3 Resource list and discovery

MCP `resources/list` returns the agent's currently scoped memories. For a memvault server, "scoped" means:

- Scoped to the agent's identity (its grants).
- Filtered by an optional tag pattern passed via the tool config.
- Paginated (MCP supports cursor-based pagination since 0.3).

Agents typically don't list everything; they search.

### 11.4 Resource read

`resources/read` for an attachment URI:

| URI suffix | Returns |
|---|---|
| (none) | base64-encoded bytes if `size ≤ inline_mcp_limit` (default 4 MiB), else `resource_link` to a chunked stream |
| `/text` | extracted text as plain text (UTF-8) |
| `/metadata` | JSON serialization of `AttachmentManifest` |
| `/findings` | JSON of `PiiFindingsBlock` if the agent holds `Action::Audit` |

For attachments larger than `inline_mcp_limit`, the response uses MCP's streaming resource pattern: a `resource_link` with metadata, and the host fetches chunks via `resources/read` with byte ranges (MCP 0.4+ supports range reads).

### 11.5 Tool surface

Tools mirror `MemvaultClient` for the write side:

```jsonc
{
  "tools": [
    {
      "name": "attach_file",
      "description": "Store a file as an attachment. Returns the manifest CID.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "filename":       { "type": "string" },
          "mime_type":      { "type": "string" },
          "content_base64": { "type": "string" },
          "tags":           { "type": "array", "items": { "type": "string" } },
          "classification": { "enum": ["public", "internal", "confidential"] },
          "visibility":     { "enum": ["internal", "federated", "public"] },
          "attach_to":      { "type": "string", "description": "optional doc_id or entity_id" }
        },
        "required": ["filename", "mime_type", "content_base64", "classification", "visibility"]
      }
    },
    {
      "name": "redact_attachment",
      "description": "Run PII redaction on an attachment, producing a redacted derivative.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "manifest_cid": { "type": "string" },
          "strategy":     { "enum": ["mask", "drop", "tokenize"] }
        },
        "required": ["manifest_cid"]
      }
    },
    {
      "name": "pin_attachment",
      "description": "Pin an attachment locally so it survives GC.",
      "inputSchema": {
        "type": "object",
        "properties": { "manifest_cid": { "type": "string" } },
        "required": ["manifest_cid"]
      }
    },
    {
      "name": "check_egress",
      "description": "Check whether content may be sent to an external destination.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "target_cid":  { "type": "string" },
          "destination": { "type": "string", "description": "destination name from policy" }
        },
        "required": ["target_cid", "destination"]
      }
    }
  ]
}
```

The full tool list also includes the existing `put_doc`, `query`, `add_edge`, `retract`, `summarize_scope`, etc. from the main plan — they don't change for attachments, but the agent uses them alongside.

### 11.6 Tool size limits

`attach_file` accepts base64 content up to `mcp_inline_attach_limit_mib` (default 16). For larger files, the agent must use a two-step pattern:

1. Tool `begin_attachment_upload` → returns a session ID and a streaming endpoint URL on the MCP server.
2. Agent uploads chunks to the endpoint via separate HTTP (the MCP host handles this).
3. Tool `commit_attachment_upload` with the session ID and metadata → returns the manifest CID.

This pattern is uncomfortable in current MCP — the protocol is JSON-RPC and not great for raw byte streaming. The 2026 roadmap mentions improvements here. For v1 we accept the awkwardness; openclaw rarely needs to upload >16 MiB inline.

### 11.7 Auth and capabilities

MCP 2026 standardized OAuth 2.1 for remote servers. For memvault, the MCP server runs **co-located with the agent host** and authenticates the agent via the existing memvault agent identity (Ed25519 keypair signing a challenge). The MCP host treats memvault as a local server and inherits the agent's effective capabilities.

Concretely:

- The memvault MCP server is a process spawned by the host (stdio transport) or an HTTP server bound to localhost (for dev).
- It loads the agent's keypair from `${AGENT_DATA_DIR}/identity/agent.key` at startup.
- It uses the agent's effective grants to filter/refuse all operations.
- An attempt to `attach_file` with `classification: confidential` by an agent without `Write` on `classification:confidential/*` is refused with a structured error.

For remote MCP servers (Phase 8), OAuth 2.1 is layered in: the agent obtains a token from the cluster admin, presents it to the remote MCP server, which validates against the cluster's admin keys.

### 11.8 Streaming and progress

Long operations (extraction of a large PDF, redaction of a 200-page document) use MCP's progress notifications:

```jsonc
{ "method": "notifications/progress", "params": { "progressToken": "...", "progress": 0.4, "message": "Extracting page 80 of 200" } }
```

The agent surfaces these to the user. Without progress reporting, a 30-second extraction looks like a hang.

### 11.9 Resource subscriptions

MCP `resources/subscribe` lets the agent be notified of changes. For memvault:

- Subscribing to `memvault://search?q=...` notifies on new envelopes matching the query.
- Subscribing to `memvault://attachment/<cid>` notifies on `ManifestUpdate` events (e.g., extraction completed, PII scan ready).

This maps naturally to the existing memvault gossipsub-backed subscription stream.

### 11.10 Changes summary for the existing MCP server

If the memvault MCP server already exists for openclaw:

1. Add the new resource URI schemes (`/attachment/...`, `/document/.../attachments`).
2. Add the new tools (`attach_file`, `redact_attachment`, `pin_attachment`, `check_egress`).
3. Implement `resources/read` range support for chunked attachments.
4. Implement progress notifications for extraction, redaction, and large reads.
5. Wire `resources/subscribe` to the manifest-update gossip stream.
6. Add the `begin_attachment_upload` / `commit_attachment_upload` pair for >16 MiB files.

---

## 12. Workspace changes

Three new crates, two existing crates extended.

### New: `memvault-attach`

The attachment-storage layer. Sits beside `memvault-doc` in the layering.

```
memvault-attach/src/
├── lib.rs
├── manifest.rs         # AttachmentManifest, ManifestUpdate
├── chunk.rs            # ChunkLayout, inline-vs-unixfs decision
├── unixfs.rs           # rust-unixfs adapter (write side: file → DAG; read side: walk + range)
├── pin.rs              # PinReason, pin/unpin lifecycle
├── cache.rs            # AttachmentCache (LRU for lazy chunks)
├── replication.rs      # ReplicationHint resolution + auto-classification
├── read_range.rs       # streaming range reads via UnixFS walk
└── error.rs
```

### New: `memvault-extract`

Extraction pipeline. Pure CPU work; no networking.

```
memvault-extract/src/
├── lib.rs
├── registry.rs         # ExtractionRegistry
├── extractor.rs        # Extractor trait
├── extractors/
│   ├── plain_text.rs
│   ├── markdown.rs
│   ├── html.rs
│   ├── pdf.rs          # uses pdf-extract
│   ├── docx.rs         # uses docx-rs
│   └── csv_json.rs
├── async_queue.rs      # deferred extraction worker
└── error.rs
```

### Extended: `memvault-policy`

PII scanning gains attachment paths:

```
memvault-policy/src/pii/
├── ...
└── attachment.rs       # scan_attachment(manifest_cid)

memvault-policy/src/cleaner/
├── ...
└── attachment.rs       # redact_attachment(manifest_cid, policy)
```

### Extended: `memvault-query`

Tantivy schema v2 with attachment fields:

```
memvault-query/src/index/
├── ...
├── schema_v2.rs        # adds attachment fields
└── migration.rs        # v1 → v2 migration runner
```

### Extended: `memvault-api`

`MemvaultClient` gains attachment methods:

```rust
trait MemvaultClient {
    // ... existing methods ...

    // attachments
    async fn attach_file(&self, req: AttachRequest) -> Result<Cid>;
    async fn read_attachment(&self, cid: Cid) -> Result<AttachmentRead>;
    async fn read_attachment_range(&self, cid: Cid, start: u64, end: u64)
        -> Result<impl Stream<Item = Result<Bytes>>>;
    async fn read_extracted_text(&self, cid: Cid) -> Result<Option<ExtractedText>>;

    async fn pin_attachment(&self, cid: Cid) -> Result<()>;
    async fn unpin_attachment(&self, cid: Cid) -> Result<()>;
    async fn list_pinned(&self) -> Result<Vec<PinnedAttachment>>;

    async fn redact_attachment(&self, cid: Cid, policy: RedactionPolicy)
        -> Result<RedactionResult>;
}

struct AttachRequest {
    bytes:          Vec<u8>,         // for inline; for big files, stream API instead
    filename:       Option<String>,
    mime_type:      String,
    tags:           Vec<Tag>,
    visibility:     Visibility,
    replication:    Option<ReplicationHint>,
    attach_to:      Option<AttachmentTarget>,    // doc_id or entity_id
}

enum AttachmentTarget {
    Document(DocId),
    Entity(EntityId),
}
```

For very large uploads, a streaming variant:

```rust
async fn attach_file_streaming(&self, header: AttachHeader)
    -> Result<(UploadSession, impl Sink<Bytes>)>;
async fn commit_attachment_upload(&self, session: UploadSession) -> Result<Cid>;
```

### Extended: `memvault-web`

The webapp gains an `AttachmentDetail` component (Phase 4 of this plan). Not in Phase 7 of the main plan because it's a sibling track.

```
memvault-web/src/app/components/
├── ...
├── attachment_detail.rs       # mv-attachment-detail
└── attachment_uploader.rs     # mv-attachment-uploader
```

### Extended: `memctl`

New subcommands:

```
memctl attach --file path.pdf --tag classification:internal --visibility internal
memctl pin <manifest_cid>
memctl unpin <manifest_cid>
memctl pinned
memctl extract <manifest_cid>           # force re-extraction
memctl scan-pii <manifest_cid>          # force re-scan
memctl redact <manifest_cid> --strategy mask
memctl read <manifest_cid> [--range 0:1024]
memctl info <manifest_cid>              # show manifest as JSON
```

---

## 13. Cargo dependencies (additions)

```toml
# IPLD DAG-PB and UnixFS
ipld-dagpb           = "*"           # DAG-PB codec
rust-unixfs          = "*"           # UnixFS file DAG construction + walking

# Extraction
pdf-extract          = "*"           # text from PDFs
docx-rs              = "*"           # DOCX parsing
scraper              = "*"           # HTML to text (or html2text)

# Misc
bytes                = "*"           # for streaming reads/writes
async-stream         = "*"           # stream! macro for impl Stream
tokio-util           = { version = "*", features = ["io"] }
```

Notably absent:

- No `infer` / `tree_magic_mini` — we trust client MIME.
- No `tesseract` — OCR is out of scope.
- No vector / embedding libraries — RAG is out of scope.

---

## 14. Phase plan

This plan has its own phasing, parallel to the main plan's phases. It assumes the main plan's Phase 1–3 are complete (we need envelopes, blockstore, and the agent API trait).

### Phase A1 — Inline attachments + manifest ★ milestone

**Goal:** Attach a small file (≤64 KiB), retrieve it, see it in audit, retract it.

**Deliverables**

1. `memvault-attach`: `AttachmentManifest` data type, `ChunkLayout::Inline` path, manifest envelope handling, classification linter integration.
2. `memvault-store`: `attachment_chunks` and `pinned` tables.
3. `memvault-api`: `attach_file` and `read_attachment` methods (inline only).
4. `memctl attach`, `memctl read`, `memctl info`.
5. Audit integration: `AttachmentCreated` op kind.
6. Tests: round-trip a 1 KiB and 60 KiB file through 3 peers; classification enforced; retraction round-trip.

**Definition of done.** Inline attachments behave indistinguishably from any other signed envelope.

### Phase A2 — Chunked attachments (UnixFS)

**Goal:** Attach a 50 MB file, retrieve it, range-read it.

**Deliverables**

1. `memvault-attach::unixfs`: write side (file → DAG-PB tree) and read side (walker, range reader).
2. `ChunkLayout::UnixFs` path through manifest commit.
3. `read_attachment_range` streaming API.
4. `memctl attach` for arbitrary file sizes; `memctl read --range`.
5. Tests: 50 MB and 500 MB files round-trip; range reads return only the chunks needed; re-fetching after eviction works.

**Definition of done.** A 1 GB file attaches and ranges-reads without OOM. Kubo can read the file given its CID (interop sanity check).

### Phase A3 — Replication policy and pinning

**Goal:** Eager files auto-replicate; lazy files fetch on demand; pinning works.

**Deliverables**

1. Auto-classification by size (`eager_threshold_bytes`).
2. Eager replication on manifest gossip: pre-fetch chunks, pin.
3. `AttachmentCache` LRU for lazy chunks.
4. `pin_attachment` / `unpin_attachment` / `list_pinned` API.
5. Manual GC honors pinned set (`memctl gc` extension).
6. Tests: eager file appears on all peers within N seconds; lazy file fetches on first read; pinning prevents eviction; per-tag policy from config (Phase 3 of this plan).

### Phase A4 — Extraction and indexing

**Goal:** PDF and DOCX attachments are text-searchable in tantivy.

**Deliverables**

1. `memvault-extract`: extractor registry, default extractors (plain, markdown, html, pdf, docx, csv).
2. Synchronous-by-default extraction at write time.
3. `ManifestUpdate` envelope for adding extracted text to existing manifests.
4. Tantivy schema v2 with attachment text fields; migration runner.
5. `query` returns `HitKind::Attachment` results.
6. `memctl extract` to force re-extraction.
7. Async extraction worker for files > `extract_async_threshold_mb`.
8. Tests: PDF text indexed and searchable; DOCX text indexed; binary file gets no extracted text and a warning; query results interleave docs and attachments by relevance.

### Phase A5 — PII scanning, redaction, egress for attachments

**Goal:** Attachments pass through the same PII/egress pipeline as documents.

**Deliverables**

1. `memvault-policy::pii::attachment`: `scan_attachment`.
2. `memvault-policy::cleaner::attachment`: `redact_attachment` (text-only output).
3. Egress policy extended for attachment-specific cases (unscannable content, not-fully-fetched).
4. `check_egress` works on attachment manifest CIDs.
5. `memctl scan-pii`, `memctl redact`.
6. Tests: PDF with email and SSN scanned, findings produced; redact produces a `text/plain` derivative; original unmodified; egress gate refuses confidential PDF without redaction; refuses lazy attachment that isn't pinned.

### Phase A6 — MCP integration

**Goal:** openclaw can use attachments via MCP.

**Deliverables**

1. New MCP resource URI schemes (`memvault://attachment/...`).
2. `resources/read` for attachments, with size-based inline-vs-link decision.
3. `resources/read` range support.
4. New tools: `attach_file`, `redact_attachment`, `pin_attachment`, `check_egress`.
5. Two-step upload tools (`begin_attachment_upload` / `commit_attachment_upload`) for >16 MiB.
6. Progress notifications for extraction and large reads.
7. Resource subscriptions for manifest updates.
8. Tests: openclaw can attach a 5 MB PDF, search across its text, retrieve a 500-byte range, request redaction, send the redacted output to a configured cloud destination, and observe audit records.

**Definition of done.** A canonical openclaw flow ("Here's a PDF, summarize it after redacting PII") works end-to-end via MCP.

### Phase A7 — Webapp surface

**Goal:** Operators can browse, preview, redact, pin attachments from the webapp.

**Deliverables**

1. `AttachmentDetail` component: metadata, MIME-aware preview (images inline, PDFs via embed, plain text rendered, others as download link), classification badge, PII findings tab.
2. `AttachmentUploader` component: drag-and-drop, classification chooser, visibility chooser.
3. List integration: `MemoryList` rows for attachments with type icon and size.
4. `EgressGate` works on attachment URIs.
5. Pin/unpin/redact buttons with capability checks.

### Phase A8 — Operations and hardening

Bloom-filter heartbeat on `ai-memvault/attachments/v1`. Per-tag-scope replication policy (TOML). OCR (feature-gated). PDF-with-redacted-regions output (if pdf-writer is mature). Streaming MCP upload over WebSocket. Vector embeddings + similarity search (out of scope of this plan; mentioned for completeness).

---

## 15. Open decisions

| Decision | Default | Revisit at | Notes |
|---|---|---|---|
| Inline threshold | 64 KiB | A2 capacity | Below bitswap msg limit |
| UnixFS chunk size | 256 KiB | A2 capacity | Matches kubo |
| UnixFS layout | Balanced | A2 | Trickle better for streaming reads of large files |
| Eager replication threshold | 10 MiB | A3 | Cluster size sensitive |
| Attachment cache size | 5 GiB | A3 | Per-peer disk budget |
| MCP inline attach limit | 16 MiB | A6 | MCP awkwardness for big files |
| MCP inline read limit | 4 MiB | A6 | Conservative; LLMs choke on huge bytes anyway |
| Async extraction threshold | 50 MiB | A4 | Tune from real workloads |
| OCR | none | A8 | Tesseract is heavy |
| Redacted PDF output | none (text only) | A8 | Pending pdf-writer maturity |
| Vector embeddings | none | A8+ | Out of scope of this plan |

---

## 16. Risks

- **`pdf-extract` quality.** PDFs are notoriously hostile to text extraction (scans, complex layouts, embedded fonts). Expect ~80% success rate on real-world PDFs. Phase A8 should evaluate if `pdfium-render` (with native deps) is worth the upgrade.
- **`rust-unixfs` API stability.** The crate is functional but pre-1.0. Isolate behind our `memvault-attach::unixfs` module so a future swap is local.
- **Trust-the-client MIME risk.** Wrong MIME → wrong extractor → no extracted text → unscanned → potentially leaks PII through egress. Mitigation: egress policy can require that attachments destined for high-stakes destinations have *successful* extraction; refuse otherwise. Document this as the operational rule.
- **MCP large-file awkwardness.** Two-step upload is real friction. Watch the MCP roadmap for improvements; consider WebSocket transport in A8.
- **Eager replication storage explosion.** A team uploads a 9 MB JSON file every minute and every peer eagerly stores all of them. Mitigation: per-peer config to disable eager (`eager_attachment_max_size_mb = 0`); per-tag-scope opt-out; quotas on eager-replicated bytes per peer per day.
- **Attachment cache thrashing.** Frequent reads of files just over the cache size cause thrash. Mitigation: cache size is operator-tunable; metrics surface the eviction rate.
- **PII false negatives on extracted text.** Extraction sometimes loses formatting that helps the detector (line breaks within phone numbers, table cells with names). Treat detector output as a backstop; don't rely on it as ground truth.
- **Federation eager-pinning explosion.** Federation peers honoring `Eager` would balloon their disk. Mitigation: federation peers always treat attachments as `Lazy` regardless of hint.
- **Retraction of attachments doesn't free chunks.** Retracted manifests are tombstoned, but their chunks remain referenced (by the `attachment_chunks` table) until GC. A regulatory-purge tool (Phase 8 of main plan) needs to know to remove chunks not referenced by any non-retracted manifest.
- **Provenance loops.** A redacted attachment's `provenance` points at the source. The source is `kind:attachment`; the redacted is `kind:redacted-attachment`. We must NOT fold extracted text or PII findings of the source into the redacted derivative — those refer to the *source*'s text, not the redacted output. The current shape avoids this because `extracted_text` is per-manifest, not transitive.
- **Manifest update divergence.** Two peers add `ExtractedText` for the same attachment using different extractors at the same time. Both `ManifestUpdate` envelopes are valid; the materialized state takes the latest by wall_ns. Edge case: clock skew. Mitigation: extraction is idempotent enough that two extractors producing different text is mostly cosmetic; both are kept in history.

---

## 17. First-week tasks

1. Skeleton `memvault-attach` crate with `AttachmentManifest`, `ChunkLayout`, classification linter integration.
2. Implement inline-only path (≤64 KiB): `attach_file`, `read_attachment` going through the existing blockstore.
3. Add `attachment_chunks` and `pinned` redb tables to `memvault-store`.
4. Wire `MemvaultClient::attach_file` and `read_attachment` (inline path).
5. `memctl attach`, `memctl read`, `memctl info` for inline path.
6. Audit projection: `AttachmentCreated` op kind populated correctly.
7. Two-node smoke test: attach a 1 KiB markdown file as attachment from peer A, read it from peer B, verify the manifest signature, classification tag, and audit record on both peers.

Phase A1 milestone work flows from there.
