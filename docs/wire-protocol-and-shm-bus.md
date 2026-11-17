# Wire Protocol, `/dev/shm` Telepathy Bus & MCP Virtual Pager Specification

**References:** [docs/architecture.md](architecture.md) §2, [ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md](../ref-docs/novel-methods/non-human-neural-codecs-and-holographic-context.md) §5  
**Target Crate:** `crates/icx-bus`, `crates/icx-adapters`

---

## 1. Purpose

When multiple agents (e.g., Claude Code editing backend Rust code, Codex running test suites, Antigravity CLI designing architecture docs, and Hermes managing background tasks) run concurrently on the same machine, they must share working memory at **memory-bus speed (`< 100 microseconds`)** without lock contention or database-locking errors (`SQLITE_BUSY`).

`crates/icx-bus` provides:
1. A lock-free, memory-mapped **POSIX Shared-Memory Seqlock Ring Buffer (`/dev/shm/icx-fabric-v1`)**.
2. A low-latency **Unix Domain Socket (`UDS`) RPC framing protocol** for cold CAS dereferences and graph queries.
3. A **Single-Tool Polymorphic MCP Pager (`icx`)** that gives every MCP-capable agent full virtual-memory syscall access at `< 180 tokens` of schema cost.

---

## 2. `/dev/shm/icx-fabric-v1` Lock-Free Seqlock Ring Layout

The shared memory file `/dev/shm/icx-fabric-v1` (or `$XDG_RUNTIME_DIR/icx-fabric-v1.shm`) has a fixed size of `1,048,640` bytes (`64 B` header + `256 x 4,096 B` slots) mapped via `mmap(PROT_READ | PROT_WRITE, MAP_SHARED)`:

```text
Offset 0x0000_0000 .. 0x0000_003F (64 B):     FabricHeader (Magic, Version, RootSeqlock, WriteSeq, ActiveAgentsMask, GlobalDagRoot)
Offset 0x0000_0040 .. 0x0010_003F (1,048,576 B): SeqlockSlotRing (256 Slots x 4,096 Bytes)
```

### 2.1 `FabricHeader` & `SeqlockSlot` Layout (`#[repr(C, align(64))]`)

```rust
pub const ICX_SHM_MAGIC: u64 = 0x4943_585F_4641_4231; // "ICX_FAB1"
pub const ICX_RING_SLOTS: usize = 256;

#[repr(C, align(64))]
pub struct FabricHeader {
    pub magic: std::sync::atomic::AtomicU64,
    pub version: u32,
    pub root_seqlock: std::sync::atomic::AtomicU32,
    /// Monotonically increasing global sequence counter (AtomicU64)
    pub write_seq: std::sync::atomic::AtomicU64,
    /// Bitmask of active AgentKind variants attached to the fabric
    pub active_agents_mask: std::sync::atomic::AtomicU64,
    /// Latest sealed global Merkle-DAG root hash (seqlock-versioned via `root_seqlock`)
    pub global_dag_root: [std::sync::atomic::AtomicU64; 4],
}

#[repr(C, align(64))]
pub struct SeqlockSlot {
    /// Odd while writer is mutating; even (= (seq + 1) * 2) when committed
    pub seqlock: std::sync::atomic::AtomicU64,
    pub timestamp_ms: u64,
    pub artifact_id: u64,
    pub workspace_hash: u64,
    pub agent_kind: u8,
    pub surprise_q8: u8,
    pub payload_len: u16,
    pub packet_kind: u8,
    pub _reserved: [u8; 3],
    /// Compact 256-bit sketch of the 10,240-bit HDC hypervector
    pub hdc_sketch: [u64; 4],
    /// Quantized 16-D Fractal Chaos-Game coordinate (i16 fixed-point in [-32767, 32767])
    pub fractal_q15: [i16; 16],
    pub _pad: [u8; 24],
    /// Inline UTF-8 AHTL Holographic Packet (up to 3,968 bytes)
    pub ahtl_bytes: [u8; 3968],
}
```

### 2.2 Lock-Free Seqlock Read/Write Protocol
- **Writer (`publish_slot`):**
  1. Atomically fetch-adds `header.write_seq` to claim sequence number `seq`.
  2. Computes `slot_idx = (seq as usize) % ICX_RING_SLOTS`.
  3. Stores `(seq * 2) | 1` (odd) into `slot.seqlock` with `Ordering::Release`, followed by `compiler_fence(Ordering::AcqRel)`.
  4. Writes metadata and `ahtl_bytes`.
  5. Stores `(seq + 1) * 2` (even) into `slot.seqlock` with `Ordering::Release`.
- **Reader (`read_since_seq`):**
  1. Loads `s1 = slot.seqlock.load(Ordering::Acquire)`. If `s1 & 1 != 0`, spins/skips.
  2. Copies the `SeqlockSlot` payload.
  3. Issues `fence(Ordering::Acquire)` and loads `s2 = slot.seqlock.load(Ordering::Relaxed)`.
  4. Accepts the read if and only if `s1 == s2`.
- **Zero-Wait Guarantee:** Writers never block on readers; readers never block writers; a crashed process can never leave a mutex deadlocked!

---

## 3. Single-Tool Polymorphic MCP Pager (`icx`)

Installing 15 separate MCP tools bloats every agent's system prompt by 3,000–6,000 tokens. Following the `mcptoon` virtualization principle, `icx mcp-serve` registers **exactly one ultra-compact polymorphic tool** named `icx`:

```json
{
  "name": "icx",
  "description": "Infinity-Context MMU syscall: deref #icx:<id> handles, slice lines, regex cold storage, query bi-temporal graph, or broadcast cross-agent state.",
  "inputSchema": {
    "type": "object",
    "required": ["op"],
    "properties": {
      "op": {
        "type": "string",
        "enum": ["deref", "slice", "grep", "recall", "fold", "xagent", "status"]
      },
      "id": { "type": "string", "description": "#icx:<hex> handle" },
      "q": { "type": "string", "description": "Query, regex, or AHTL payload" },
      "range": {
        "type": "array",
        "items": { "type": "integer" },
        "description": "[start_line, end_line] (1-based inclusive)"
      }
    }
  }
}
```

### Supported `op` Syscalls
- `op: "deref"`: Returns the full lossless raw text of `id` (`#icx:<hex>`).
- `op: "slice"`: Returns lines `range[0]..=range[1]` of `id`.
- `op: "grep"`: Runs fast regex `q` across `id` (or across all cold artifacts in the current workspace if `id` is omitted) and returns matching lines with context.
- `op: "recall"`: Executes a combined 10,240-bit HDC + HippoRAG PPR + Fractal trajectory search for `q` and returns matching AHTL packets and `#icx:<hex>` handles.
- `op: "fold"`: Explicitly folds a completed sub-task into an `L2 SubTrajectoryBranch` node.
- `op: "xagent"`: Reads or broadcasts live telepathy packets across all agents on `/dev/shm/icx-fabric-v1`.
- `op: "status"`: Returns current Merkle-DAG depth, Cache-Epoch count, compression ratio, and `/dev/shm` ring health.
