# Core Data Model & Storage Layout Specification

**Applies to:** `icx` v1.2.0 candidate

**References:** [docs/architecture.md](architecture.md), [docs/virtual-memory-and-dag-engine.md](virtual-memory-and-dag-engine.md), [docs/hyperdimensional-and-fractal-index.md](hyperdimensional-and-fractal-index.md)  
**Target Crates:** `crates/icx-core`, `crates/icx-hdc`, `crates/icx-codec`, `crates/icx-store`, `crates/icx-adapters`

The type definitions below are abridged from the code (attributes and private fields omitted).
Where this document and the code disagree, the code is authoritative.

---

## 1. Fundamental Identifiers & Value Types (`crates/icx-core`)

```rust
/// 64-bit handle: the first 8 big-endian bytes of a BLAKE3 digest.
/// Rendered as `#icx:<16-lowercase-hex>` (Profile B) or `#icx:<12-lowercase-hex>` (48-bit, Profile A).
pub struct ArtifactId(pub u64);

/// Full 256-bit BLAKE3 digest.
pub struct Blake3Digest(pub [u8; 32]);

#[repr(u8)]
pub enum AgentKind {
    ClaudeCode = 1,
    Codex = 2,
    AntigravityCli = 3,
    Hermes = 4,
    GeminiCli = 5,
    OpenCode = 6,
    Cursor = 7,
    GenericMcp = 255,
}

pub enum IcxError {
    // ... invalid handle, I/O, serialization, size-limit variants ...
    /// Stored bytes do not match their BLAKE3 digest.
    IntegrityViolation(ArtifactId),
    /// A different payload already occupies this 64-bit handle; nothing was overwritten.
    HashCollision(ArtifactId),
}
```

`ArtifactId` is a 64-bit truncation of a 256-bit digest. Collisions are detected by comparing full
digests and reported as `HashCollision`; the colliding payload is not virtualized.

---

## 2. Content-Addressed Store (`crates/icx-core/src/cas.rs`)

Chunks are stored as raw bytes (no compression).

```rust
pub struct LineIndexEntry {
    pub start_line: u32,
    pub end_line: u32,
    pub byte_offset: u32,
    pub byte_len: u32,
    pub chunk_id: ArtifactId,
}

pub struct StoredArtifact {
    pub id: ArtifactId,
    pub full_digest: Blake3Digest,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub chunk_ids: Vec<ArtifactId>,
    pub line_index: Vec<LineIndexEntry>,
}

pub struct CasStore {
    chunk_config: ChunkerConfig,                               // FastCDC 2 KiB / 8 KiB / 64 KiB
    chunks: BTreeMap<ArtifactId, (Blake3Digest, Vec<u8>)>,     // L0 chunks
    artifacts: BTreeMap<ArtifactId, StoredArtifact>,
}
```

---

## 3. Merkle-DAG Node Models (`crates/icx-core/src/dag.rs`)

```rust
pub struct ToolArtifactNode {
    pub id: ArtifactId,            // derived from dag_digest
    pub dag_digest: Blake3Digest,  // domain-separated b"ICX_DAG_TOOL_V1" digest over all fields
    pub raw_artifact: ArtifactId,  // CAS handle of the stored (redacted) payload
    pub full_digest: Blake3Digest,
    pub agent: AgentKind,
    pub session_id: String,
    pub tool_name: String,
    pub exit_code: Option<i32>,
    pub total_bytes: usize,
    pub total_lines: usize,
    pub chunks: Vec<ArtifactId>,
    pub line_index: Vec<LineIndexEntry>,
    pub ahtl_stub: String,
    pub surprise_score: f32,
    pub created_at_ms: u64,
    pub token_estimate: usize,
}

pub enum DagNode {
    /// L1: one conversational turn.
    Turn { id, digest, ordinal: u64, agent, role: String, raw_artifact, ahtl_repr: String,
           surprise: f32, token_estimate: usize },
    /// L1: one tool execution.
    ToolArtifact(ToolArtifactNode),
    /// L2: a folded sub-trajectory branch (Context-Folding, arXiv:2510.11967).
    SubTrajectoryBranch { id, digest, label: String, child_turns: Vec<ArtifactId>,
                          folded_ahtl_commit: String, raw_tokens_saved: usize },
    /// L3/L4: a sealed cache epoch.
    CacheEpoch { epoch_index: u32, id, merkle_root: Blake3Digest, child_nodes: Vec<ArtifactId>,
                 raw_tokens: usize, compressed_tokens: usize, ahtl_frame: String },
}
```

Inserting a node whose id is already held by a *different* node is refused (ICX-AUD-023).

Event timestamps (`created_at_ms`, graph edge times) come from a logical counter by default, which
keeps library use and tests deterministic. The CLI enables wall-clock time
(`InfinityContextMmu::use_wall_clock(true)`; strictly increasing milliseconds since the Unix
epoch) for real workspaces (ICX-AUD-048).

---

## 4. AHTL Codec Data Model (`crates/icx-codec/src/ahtl.rs`)

```rust
pub enum PacketKind {
    CodeDelta,     // Δ: code / AST / file read or mutation
    ToolStub,      // Ψ: tool execution output or diagnostic stub
    Invariant,     // Ω: architectural invariant or constraint
    Telepathy,     // Θ: cross-agent broadcast
    FoldedBranch,  // Φ: folded sub-trajectory branch commit
}

pub enum ClauseOp { Added, Removed, Negated, Anomaly, Verified, Neutral } // ⊕ ⊖ ¬ ! ✓ (none)

pub struct AhtlClause {
    pub op: ClauseOp,
    pub key: String,
    pub value: Option<String>,
    /// Causal `⇒` implication; parsing materializes at most MAX_CLAUSE_CHAIN (64) links.
    pub implies: Option<Box<AhtlClause>>,
}

/// Profile B packet.
pub struct AhtlPacket {
    pub kind: PacketKind,
    pub subject: String,
    pub line_range: Option<(u32, u32)>,
    pub clauses: Vec<AhtlClause>,
    pub deref_id: Option<ArtifactId>,
    pub compact_48bit_handle: bool,
    /// Must match SliceMap (`L<a>-<b>` | `B<a>:<b>`); invalid hints are not rendered.
    pub slice_hint: Option<String>,
    pub hologram_bytes: [u8; 8],
    pub hologram_tag: Option<u16>,
}

/// Profile A packet: `⟦AHTL v1│#icx:<12|16-hex>((<slice_hint>))?│fc:<64-hex>│hv:<16-hex>│<Braille-64>│<Skeleton>⟧`.
pub struct AhtlWirePacketV1 {
    pub cas_handle: ArtifactId,
    pub compact_48bit_handle: bool,
    pub slice_hint: Option<String>,
    pub fractal_coord_hex: String,
    /// Linear, non-cryptographic sketch of the hypervector; not an identity (ICX-AUD-053).
    pub hdc_digest_hex: String,
    pub braille_spectrum: [u8; 64],
    pub skeleton_body: String,
}

/// `⟪ICX:v1│ep=<4-8 hex>│root=<8|16 hex>│(τ|nodes|folded)=<u32>⟫`
pub struct EpochHeader {
    pub epoch_id: u32,            // hexadecimal on the wire: `ep=000a` is epoch 10
    pub merkle_root_hex: String,
    pub count_kind: EpochCountKind, // TurnStart(u32) | Nodes(u32) | Folded(u32)
}
```

Profile A `fc` words keep the shipped signed Q15 scale 32,767; Q1.14 is a separate arithmetic
API and must not reinterpret archived packets. Rendering normalizes raw skeleton packet/header
glyphs to ASCII bracket pairs, and standalone parsing rejects unescaped nested framing.
Skeletons are lossy views; exactness belongs to post-redaction CAS dereference.

The 8-byte `hologram_bytes` (`ChannelBHologram`) layout is: `B[0..=3]` HDC parity fold,
`B[4..=5]` IFS quadrant signature, `B[6]` quantized surprise, `B[7]` flags (bits 0-1 retention
tier, bit 2 `FLAG_HAS_ERROR`, bit 3 `FLAG_HAS_PANIC`, bit 4 `FLAG_HAS_DIFF`, bit 5
`FLAG_IS_FOLDED`, bit 6 `FLAG_IS_EPOCH`, bit 7 `FLAG_IS_REDACTED`; `crates/icx-codec/src/braille.rs`).

---

## 5. Bi-Temporal Knowledge Graph Schema (`crates/icx-store/src/graph.rs`)

```rust
pub struct BiTemporalEdge {
    pub edge_id: u64,
    pub source_entity: String,
    pub relation: String,
    pub target_entity: String,
    /// Functional relations (e.g. `return_type`, `test_status`) invalidate the prior target.
    pub is_functional: bool,
    /// Valid time `[t_valid, t_invalid)` in ms.
    pub t_valid: u64,
    pub t_invalid: Option<u64>,
    /// Transaction time `[t_tx_start, t_tx_end)` in ms.
    pub t_tx_start: u64,
    pub t_tx_end: Option<u64>,
    /// CAS artifact the edge was extracted from.
    pub provenance: ArtifactId,
    pub weight: f32,
}
```

---

## 6. Workspace Storage Layout (`crates/icx-adapters/src/store.rs`, `workspace.rs`)

A workspace directory (mode `0700`, owned by the user) contains:

| File | Purpose |
| :--- | :--- |
| `icx.sqlite3` (+ `-wal`, `-shm`) | The workspace store (SQLite, WAL journaling, `synchronous=NORMAL`, mode `0600`). |
| `OWNER` | Owner seal: keyed BLAKE3 MAC of the canonical directory path under the install key. |
| `.gitignore` | Always `*`. |
| `cas_store.lock` | Advisory workspace lock taken by every writer. |
| `icx-fabric-v1.shm` | Default file-backed telepathy ring (unless `ICX_SHM_PATH` is set). |

v1.0.x JSON files (`cas_store.json`, `mmu_state.json`, `bitemporal_graph.json`) found in a trusted
workspace are migrated into the store on first load and left in place.

### 6.1 SQLite Schema

```sql
CREATE TABLE meta      (k TEXT PRIMARY KEY, v BLOB NOT NULL);
CREATE TABLE chunks    (id INTEGER PRIMARY KEY, digest BLOB NOT NULL, bytes BLOB NOT NULL);
CREATE TABLE artifacts (id INTEGER PRIMARY KEY, meta TEXT NOT NULL); -- StoredArtifact as JSON
CREATE TABLE emitted   (digest BLOB PRIMARY KEY) WITHOUT ROWID;    -- BLAKE3 of each icx-emitted block
```

| `meta` key | Content |
| :--- | :--- |
| `schema` | Schema version (`1`); other versions are refused. |
| `generation` | Monotonic save counter; lets long-lived processes skip reloading. |
| `chunk_config` | `ChunkerConfig` (JSON). |
| `graph` | Bi-temporal graph (JSON). |
| `mmu_state` | DAG, epoch controller (including versioned `window_budget`) and MMU tables (JSON). |
| `hdc_index` | HDC associative index, binary snapshot format `ICXHDC05`, including exact token postings. |
| `hdc_index_events` | Number of events the snapshot covers. |
| `seal` | Store MAC (§6.2). |

`chunks`, `artifacts` and `emitted` rows are append-only (inserted once). Existing store, `-wal`
and `-shm` files are reset to `0600` on open if their mode was widened. The derived state in `meta` is
replaced on each save. Each save is one WAL transaction with `synchronous=FULL`; the daemon
sends success only after it commits. Save failure is returned to the client, never acknowledged;
from v1.2.1 the daemon then discards its unsaved in-memory state and keeps serving, exiting with an
error only after 5 consecutive save failures. A failed dispatch is not saved at all.

### 6.2 Store Seal

`seal = BLAKE3_keyed(store_key, domain ‖ canonical_path ‖ generation ‖ state_digest)`, where
`store_key` is derived from the install key and `state_digest` is a BLAKE3 digest over the chunk
config, graph, MMU state, HDC snapshot, HDC event count, every artifact's `(id, full_digest)` in
id order, and every `emitted` digest in sorted order. Chunks are covered transitively, because loading verifies every chunk digest and
reassembles each artifact to its full digest. A sealed load rejects a store whose MAC does not
verify.

### 6.3 Snapshot and Window-Policy Compatibility

The HDC decoder reads `ICXHDC05` and legacy `ICXHDC04`/`ICXHDC03`. 05 uses the v1.2.1 tokenizer
(runs joined by `:` or `/` are also posted by component); 04 postings come from the older tokenizer
and are validated, then dropped, so the MMU rebuilds them from CAS events, as for 03, which has no
postings at all. Version 04 appended optional exact
token-to-entry postings to the index, summary, trajectory and surprise state. It checks posting
ordering/ranges, finite numeric domains and consistency of derived caches. The workspace MAC
remains the provenance boundary; a self-consistent arbitrary snapshot is not authenticated by
these checks alone. Legacy 03 lacks content tokens, so the MMU rebuilds from persisted CAS events
instead of taking the 04 fast path. A standalone 03 index uses exhaustive refinement. Subsequent
saves write 04; older executables cannot read it. Keep a pre-upgrade backup for downgrade.

The epoch controller persists `window_budget` with version 1, `context_tokens`,
`completion_reserve` and `external_input_reserve`. Legacy JSON without this field receives the
default 32,768 / 4,096 / 4,096 policy. `utf8-bytes-upper-bound-v1` treats one UTF-8 byte as one token
of allowance, leaving 24,576 bytes for icx-managed prompt content under the default policy.
Reservations outside this view remain the caller's responsibility. Selection changes never
delete CAS/DAG history or reinterpret archived Q15 `fc` fields.
