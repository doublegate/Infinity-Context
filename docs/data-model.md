# Core Data Model & Storage Layout Specification

**References:** [docs/architecture.md](architecture.md), [docs/virtual-memory-and-dag-engine.md](virtual-memory-and-dag-engine.md), [docs/hyperdimensional-and-fractal-index.md](hyperdimensional-and-fractal-index.md)  
**Target Crates:** `crates/icx-core`, `crates/icx-hdc`, `crates/icx-codec`, `crates/icx-store`

---

## 1. Fundamental Identifiers & Value Types

```rust
/// 64-bit short content-addressed handle derived from the leading 8 bytes of a BLAKE3 digest.
/// Formatted externally as `#icx:<16-lowercase-hex>`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ArtifactId(pub u64);

/// Full 256-bit cryptographic BLAKE3 digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Blake3Digest(pub [u8; 32]);

/// Supported agent harnesses with native or generic interception.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
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
```

---

## 2. Merkle-DAG Node Models (`crates/icx-core`)

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeafChunk {
    pub id: ArtifactId,
    pub digest: Blake3Digest,
    pub uncompressed_len: u32,
    pub compressed_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineIndexEntry {
    pub start_line: u32,
    pub end_line: u32,
    pub byte_offset: u32,
    pub byte_len: u32,
    pub chunk_id: ArtifactId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolArtifactNode {
    pub id: ArtifactId,
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CacheEpochNode {
    pub epoch_index: u32,
    pub id: ArtifactId,
    pub merkle_root: Blake3Digest,
    pub child_nodes: Vec<ArtifactId>,
    pub raw_token_count: usize,
    pub compressed_token_count: usize,
    pub ahtl_frame: String,
    pub sealed_at_ms: u64,
}
```

---

## 3. AHTL Codec Data Model (`crates/icx-codec`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketKind {
    /// `Δ`: Code / AST / File mutation or read
    CodeDelta = b'D',
    /// `Ψ`: Tool execution / diagnostic stub
    ToolStub = b'P',
    /// `Ω`: Architectural invariant / hard rule
    Invariant = b'O',
    /// `Θ`: Cross-agent telepathy broadcast
    Telepathy = b'T',
    /// `Φ`: Folded sub-trajectory branch commit
    FoldedBranch = b'F',
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClauseOp {
    /// `⊕`: Added / defined
    Added,
    /// `⊖`: Removed / invalidated
    Removed,
    /// `¬`: Negative constraint / forbidden
    Negated,
    /// `!`: High-surprise anomaly / error / panic
    Anomaly,
    /// `✓`: Verified / test passed
    Verified,
    /// Plain key/value or relational fact
    Neutral,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AhtlClause {
    pub op: ClauseOp,
    pub key: String,
    pub value: Option<String>,
    pub causal_chain: Vec<AhtlClause>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AhtlPacket {
    pub kind: PacketKind,
    pub subject: String,
    pub line_range: Option<(u32, u32)>,
    pub clauses: Vec<AhtlClause>,
    pub deref_id: Option<ArtifactId>,
    pub slice_hint: Option<String>,
    pub hologram_bytes: [u8; 8],
    pub hologram_tag: Option<u16>,
    pub compact_48bit_handle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AhtlWirePacketV1 {
    pub handle: ArtifactId,
    pub compact_48bit_handle: bool,
    pub fractal_coord: FractalCoord,
    pub hdc_digest: u64,
    pub braille_spectrum_64: [u8; 64],
    pub skeleton_payload: String,
}
```

---

## 4. Bi-Temporal Knowledge Graph Schema (`crates/icx-store`)

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BiTemporalEdge {
    pub edge_id: u64,
    pub source_entity: String,
    pub relation: String,
    pub target_entity: String,
    /// Functional relations (e.g., `return_type`, `test_status`) auto-invalidate prior targets
    pub is_functional: bool,
    /// Valid time interval `[t_valid, t_invalid)` in Unix ms
    pub t_valid: u64,
    pub t_invalid: Option<u64>,
    /// Transaction time interval `[t_tx_start, t_tx_end)` in Unix ms
    pub t_tx_start: u64,
    pub t_tx_end: Option<u64>,
    /// Lossless CAS artifact proving this edge
    pub provenance: ArtifactId,
    pub weight: f32,
}
```
