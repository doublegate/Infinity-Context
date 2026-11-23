# Telepathy Bus & MCP Virtual Pager Specification

**References:** [docs/architecture.md](architecture.md) §2, [docs/adr/0004-zero-copy-shm-telepathy-bus.md](adr/0004-zero-copy-shm-telepathy-bus.md), [docs/adr/0005-v1-1-0-workspace-trust-store-and-daemon.md](adr/0005-v1-1-0-workspace-trust-store-and-daemon.md), [docs/security-and-threat-model.md](security-and-threat-model.md)  
**Target Crates:** `crates/icx-bus`, `crates/icx-adapters`, `crates/icx-cli`

---

## 1. Purpose

When several agents (for example Claude Code and Codex) work in the same project, `icx` lets one
agent's high-surprise events (a compiler panic, a failing test) reach the others' next prompt.
`crates/icx-bus` provides the **telepathy ring** (`SeqlockTelemetryBus`): a fixed 256-slot ring of
event records with seqlock-validated reads.

`icx` v1.1.0 contains:

1. The **telepathy ring** (`crates/icx-bus/src/ring.rs`), with an in-process backend and a
   file-backed backend (§2).
2. An **MCP pager** (`icx mcp-serve`, `crates/icx-adapters/src/mcp.rs`) that gives MCP clients
   dereference, slicing, search, recall and bus polling through the read-only `icx` tool, and
   folding and bus broadcasts through `icx_write` (§3, ADR-0009).

The resident daemon's Unix socket (`icx daemon`, `crates/icx-cli/src/daemon.rs`) carries hook
requests only; it is described in [docs/architecture.md](architecture.md) §3.3. There is no
separate RPC protocol for CAS or graph queries.

### 1.1 What the bus is not

The v1.0.0 documentation described the bus as a lock-free, memory-mapped, zero-copy `/dev/shm`
segment (audit finding ICX-AUD-046). That does not match the code:

- The **file-backed** backend is **not memory-mapped**. It uses positional I/O (`pread`/`pwrite`
  through `std::os::unix::fs::FileExt::{read_exact_at, write_all_at}`) on an ordinary file.
  `mmap` would require `unsafe`, which the workspace forbids.
- It is **not lock-free** for writers. A file-backed writer takes an in-process `Mutex` and then an
  exclusive advisory file lock (`File::try_lock`, retried up to 5,000 times at 50 µs, about
  250 ms, before failing). Readers take no lock.
- It is **not zero-copy**. Every publish and every read copies the slot through a buffer.
- The file is wherever it is configured. The default is `icx-fabric-v1.shm` inside the
  workspace directory; `ICX_SHM_PATH` overrides it. It is on the `/dev/shm` tmpfs only if
  `ICX_SHM_PATH` points there.

---

## 2. Telepathy Ring

### 2.1 Backends

| Backend | Constructor | Scope | Synchronization |
| :--- | :--- | :--- | :--- |
| `BackingStore::InProcess` | `SeqlockTelemetryBus::new_in_memory()` | Threads of one process | Atomic seqlock per slot; writers claim a sequence number with `fetch_add` and a slot with compare-and-swap. No mutex, no syscalls. |
| `BackingStore::SharedFile` | `SeqlockTelemetryBus::open_shared_file(path)` | All processes that open the same file | Writers: in-process `Mutex` plus exclusive advisory file lock around the whole slot write. Readers: optimistic, lock-free, seqlock version check. |

Agent hooks and the MCP server use the file-backed backend, because each hook invocation is a
separate process. The in-process backend is used by the benchmark and by tests.

### 2.2 Layout

The ring has a fixed size of `1,048,640` bytes: a 64-byte header followed by 256 slots of
4,096 bytes. All integers are little-endian.

```text
Offset 0x0000_0000 .. 0x0000_003F (64 B):        FabricHeader (magic, version, root seqlock, write_seq, active agent mask, DAG root)
Offset 0x0000_0040 .. 0x0010_003F (1,048,576 B): 256 slots x 4,096 B
```

```rust
pub const ICX_SHM_MAGIC: u64 = 0x4943_585F_4641_4231; // "ICX_FAB1"
pub const ICX_SHM_VERSION: u32 = 1;
pub const ICX_RING_SLOTS: usize = 256;
pub const ICX_AHTL_INLINE_CAP: usize = 3968; // inline AHTL payload bytes per slot
```

| Header field | Offset | Meaning |
| :--- | :--- | :--- |
| `magic` | 0 | `ICX_SHM_MAGIC` |
| `version` | 8 | `ICX_SHM_VERSION`; a mismatch fails with `BusError::UnsupportedVersion` |
| `root_seqlock` | 12 | Version counter for `global_dag_root` |
| `write_seq` | 16 | Number of committed events (next sequence number) |
| `active_agents_mask` | 24 | Bit per `AgentKind` that has published |
| `global_dag_root` | 32 | Latest 256-bit Merkle-DAG root |

Each slot holds: the seqlock word, `timestamp_ms`, `artifact_id`, `workspace_hash`, a packed
metadata word (agent kind, `surprise_q8`, payload length, packet kind), a 256-bit HDC sketch, a
16-dimensional Q15 fractal coordinate, padding, and up to 3,968 bytes of UTF-8 AHTL payload
(longer payloads are truncated). In-process slots are `#[repr(C, align(64))]` arrays of
`AtomicU64` words.

### 2.3 Publish and Read Protocol

**Writer, in-process (`try_publish`):**

1. `fetch_add` on `write_seq` claims sequence number `seq`; `slot = seq % 256`.
2. The writer moves the slot's seqlock from an even value to `odd = (seq << 1) | 1` with
   compare-and-swap. If the slot already holds a version `>= (seq + 1) << 1`, a later lap has
   overwritten it and the call returns `Err(BusError::SlotLapped)`. If the slot stays odd (held
   by a stalled writer) past the spin limit, it returns `Err(BusError::SlotContended)`.
3. The writer stores the payload with relaxed atomics between `AcqRel` fences, then stores
   `even = (seq + 1) << 1` with `Release`.

**Writer, file-backed (`try_publish`):** under the mutex and file lock, the writer reads
`write_seq`, writes the odd version, writes the payload, writes the even version, and only then
advances `write_seq`. Writers are serialized, so the file-backed path does not lap itself.

**Reader (`poll_since`, `poll_workspace_since`):** for each sequence number after the cursor
(at most the last 256), the reader loads the seqlock (`Acquire`); an odd or zero value is skipped;
it copies the slot, loads the seqlock again, and accepts the record only if both loads match and
the version corresponds to the expected sequence number. `poll_workspace_since` additionally
filters on `workspace_hash`.

The file-backed reader relies on `pwrite` ordering through the page cache, which holds on Linux
but is not guaranteed by POSIX.

**Crash behaviour:** a writer killed mid-publish leaves its slot odd. Readers skip it; later writers
treat it as contended and drop their event with `SlotContended` rather than block. The file lock is
an OS advisory lock and is released when the process exits.

### 2.4 Loss Signalling

v1.0.0 returned success when an event was dropped (ICX-AUD-054). v1.1.0 makes loss observable:

| API | Behaviour |
| :--- | :--- |
| `try_publish(..) -> Result<u64, BusError>` | Returns the new sequence number, or `SlotLapped` / `SlotContended` / `Io` when the event was not committed. |
| `publish(..) -> u64` | Infallible wrapper; counts every dropped event. |
| `dropped_publishes() -> u64` | Number of events this handle failed to publish. |
| `poll_since_with_loss(since) -> (Vec<TelepathyEvent>, u64)` | Events after `since`, plus the number of sequence numbers that could not be delivered (overwritten before the reader caught up, or torn and skipped). A non-zero count means the reader must not assume it saw every event. |

Tests: `crates/icx-bus/tests/loss_signalling.rs`.

### 2.5 File Open Hardening

`open_shared_file` (ICX-AUD-019):

- opens with `O_NOFOLLOW | O_CLOEXEC`, creating the file with mode `0600`;
- refuses (`BusError::InsecurePermissions`) a path that is not a regular file, is owned by another
  uid, or has any group or other permission bits, instead of silently tightening it;
- refuses a file whose header has the wrong magic (`InvalidMagic`) or version
  (`UnsupportedVersion`); a new or zero-magic file is initialized under the file lock.

### 2.6 Measured Latency

Measured on the development machine (Intel i9-10850K, Linux):

| Path | Result | Source |
| :--- | :--- | :--- |
| In-process publish | 58.3 ns per slot over 1,000 publishes | `icx benchmark` (synthetic, single thread) |
| File-backed publish to visible in a second handle | worst case 38 µs over 200 events | two-handle test modelling two agent processes (ICX-AUD-055) |

These are component measurements, not end-to-end agent latencies. A writer waiting on the file
lock can take up to about 250 ms before failing.

---

## 3. MCP Pager (`icx mcp-serve`)

`icx mcp-serve` runs a JSON-RPC 2.0 server over stdio, one JSON message per line. It registers two
tools (v1.4.0, ADR-0009), each estimated at under 180 tokens
(`McpPagerServer::per_tool_token_estimates`):

- **`icx`**, read-only, with `annotations.readOnlyHint: true`: `deref`, `slice`, `grep`,
  `recall`, `expand`, `status`, and `xagent` as a poll. Claude Code's plan mode runs a
  read-only MCP tool without holding it, and only the hint lifts that hold (T-64-007).
- **`icx_write`**, state-changing, without the hint: `fold`, and `xagent` with a message.

```json
{
  "name": "icx",
  "annotations": { "readOnlyHint": true },
  "inputSchema": {
    "type": "object",
    "required": ["op"],
    "properties": {
      "op": { "type": "string", "enum": ["deref", "slice", "grep", "recall", "expand", "xagent", "status"] },
      "id": { "type": "string" },
      "q": { "type": "string", "description": "Pattern for grep or recall." },
      "range": { "type": "array", "items": { "type": "integer" } },
      "k": { "type": "integer" },
      "provenance": { "type": "boolean" }
    }
  }
}
```

```json
{
  "name": "icx_write",
  "inputSchema": {
    "type": "object",
    "required": ["op", "q"],
    "properties": {
      "op": { "type": "string", "enum": ["fold", "xagent"] },
      "q": { "type": "string" }
    }
  }
}
```

(Descriptions omitted here; see `McpPagerServer::tool_schema`.)

**Routing:**

- A read operation sent to `icx_write` is refused with `isError: true`, naming `icx`.
- A write operation (`fold`, or `xagent` with a non-empty `q`) sent to `icx` still runs in v1.4.x
  and its reply ends with `⟪ICX:DEPRECATED│...⟫` naming `icx_write`. `icx` stops accepting them in
  v1.5.0.
- Hook adapters treat both tools as icx's own, in every agent's naming form, and never ingest
  their output.

### 3.1 Operations

- `deref`: full stored text of `id` (`#icx:<16-hex>` or `#icx:<12-hex>`), framed (§3.3).
- `slice`: lines `range[0]..=range[1]` (1-based, inclusive) of `id`, framed.
- `grep`: pattern `q` over `id`, or over all artifacts in the workspace when `id` is omitted.
- `recall`: associative recall for `q` (HDC two-stage index), returning AHTL packets and handles;
  `k` limits the result count. It advances the bus-delivery cursors, which is bookkeeping, so it
  stays on the read-only tool.
- `fold` (`icx_write`): `q = "begin:<label>"` opens a sub-trajectory branch; `q = "<summary>"`
  closes the innermost open branch into an `L2` folded node.
- `expand`: expands a folded branch.
- `xagent`: polls the telepathy ring for the current workspace (`icx`), or publishes `q` to it
  (`icx_write`).
- `status`: DAG, epoch, index and ring status.

The ring is at the configured path (§1.1).

### 3.2 Protocol Versions (ICX-AUD-008)

The server supports two protocol eras:

- **Legacy handshake.** `initialize` negotiates `2025-11-25`, `2025-06-18`, `2025-03-26` or
  `2024-11-05`: the server echoes the client's version when it is supported and otherwise answers
  with `2025-11-25`.
- **Stateless (`2026-07-28`).** `server/discover` is supported; each request carries its version in
  `_meta["io.modelcontextprotocol/protocolVersion"]`; results carry `resultType` and
  `_meta.serverInfo`. An unsupported version returns error `-32022`.

Constants: `LEGACY_PROTOCOL_VERSIONS`, `MODERN_PROTOCOL_VERSIONS` in
`crates/icx-adapters/src/mcp.rs`.

### 3.3 Provenance Framing

Dereferenced content is untrusted data. `deref` output always starts with a provenance header,
followed by the Tier-0-escaped body:

```text
⟪ICX:PROVENANCE│id=<handle>│redacted=<bool>│exactness=post_redaction_cas│trust=untrusted_data⟫
<escaped body>
```

`exactness=post_redaction_cas` states that the bytes are exactly what the CAS stored, which is the
tool output after secret redaction. `icx deref` on the command line uses the same framing by
default; `icx deref --raw` prints the exact stored bytes.

### 3.4 Bounds

| Bound | Value | Constant |
| :--- | :--- | :--- |
| MCP stdio line length | 4 MiB | `MAX_MCP_LINE_BYTES` (`crates/icx-cli/src/main.rs`) |
| Open nested `fold begin:` branches | 64 | `MAX_FOLD_DEPTH` (`crates/icx-adapters/src/mcp.rs`) |
| Hook stdin | 64 MiB + 64 KiB envelope | `MAX_STDIN_BYTES` (`crates/icx-cli/src/main.rs`) |

Tests: `crates/icx-adapters/tests/mcp_bounds.rs`; fuzz target `fuzz/fuzz_targets/mcp_jsonrpc.rs`.
