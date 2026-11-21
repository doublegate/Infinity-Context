# Virtual Memory & Lossless Merkle-DAG Engine (`LCM`) Specification

**Applies to:** `icx` v1.2.0 candidate

**References:** [ref-docs/papers/INDEX.md](../ref-docs/papers/INDEX.md) §§1.1–1.5 & 2.1, [ref-docs/research-report.md](../ref-docs/research-report.md) §4.3  
**Target Crates:** `crates/icx-core`, `crates/icx-adapters` (persistence)

---

## 1. Purpose

`crates/icx-core` implements the **Virtual Context Memory Management Unit (MMU)** core and the
**Lossless Context Management (`LCM`) BLAKE3 Merkle-DAG**. It provides:

1. **Lossless storage after redaction:** every tool output and turn is redacted for secrets
   (`SecretRedactor::redact`) and then stored byte for byte as `L0` chunks. Redaction is a
   pattern-based gate with false-positive/false-negative limits; exactness applies to its output.
   When the gate leaves input unchanged, line endings, trailing newlines and literal delimiters
   are preserved and `deref(ingest(x)) == x` (ICX-AUD-021). Payloads are capped
   at 64 MiB (`ICX_MAX_INGEST_BYTES`).
2. A tiered view of context: active verbatim turns and stubs, sealed cache epochs, the telepathy
   ring, and the content-addressed store (CAS) in the workspace's SQLite file.
3. Compaction as a deterministic Merkle-DAG fold rather than a destructive overwrite.
4. Immutable cold epochs with bounded prompt selection. Selected history can change as the
   budget is exhausted or the model policy changes, invalidating the corresponding cache prefix.

---

## 2. Content-Addressed Storage (CAS) & FastCDC Chunking

### 2.1 Content-Defined Chunking (`FastCDC`)

`icx-core` splits payloads with gear-hash **content-defined chunking** (`crates/icx-core/src/chunker.rs`):

- Minimum chunk size: `2,048` bytes
- Target (average) chunk size: `8,192` bytes
- Maximum chunk size: `65,536` bytes

Boundaries use FastCDC normalized chunking with two masks: a stricter mask before the target size
and a relaxed mask after it. Since v1.1.0 the masks are **spread masks** whose set bits are
distributed over bit positions 63..16, as in Xia et al. (USENIX ATC 2016), instead of low-bit
masks. Low-bit masks only see about the last 14 input bytes of the gear hash. On this
repository's sources the mean chunk size moved from 13.2 KiB to 10.6 KiB (target 8 KiB)
(ICX-AUD-027). Chunks are stored uncompressed.

Each chunk is identified by its 256-bit `BLAKE3` digest and addressed by a 64-bit `ArtifactId`
(first 8 bytes of the digest, rendered `#icx:<16-hex>`).

### 2.2 Lossless Dereference Contract

Given a handle `#icx:<16-hex>` (or a 48-bit `#icx:<12-hex>` prefix):

- `CasStore::deref_bytes(id) -> Result<Vec<u8>, IcxError>` concatenates the artifact's `L0`
  chunks, verifying each chunk digest and the artifact's full 256-bit digest; any mismatch fails
  with `IcxError::IntegrityViolation`.
- `CasStore::deref_lines(id, start_line, end_line) -> Result<String, IcxError>` returns an exact
  1-based inclusive line slice, reading only the chunks the line index points to.
- `CasStore::grep_cold(id, pattern, max_matches)` and `grep_cold_with_context` search one stored
  artifact, or all of them when `id` is `None`, and return matching lines with line numbers.

The bytes returned are the stored bytes, that is, the input after secret redaction. Model-facing
output additionally applies Tier-0 delimiter escaping (`SecretRedactor::sanitize`) and a
provenance header ([docs/wire-protocol-and-shm-bus.md](wire-protocol-and-shm-bus.md) §3.3).

### 2.3 Handle Collisions

`CasStore::put_bytes_checked` compares full 256-bit digests whenever a 64-bit artifact or chunk id
is already present. A different payload with the same id fails with `IcxError::HashCollision`; the
store is not modified and the MMU does not virtualize that payload (the agent keeps its original
output). `MerkleContextDag` likewise refuses to replace a node with a different node under the
same id (ICX-AUD-022, ICX-AUD-023).

---

## 3. Hierarchical Merkle-DAG (`MerkleContextDag`)

```mermaid
flowchart BT
    L0_1["L0 chunk #icx:10a1f3c2d4e5b6a7 (bytes 0..8192)"]
    L0_2["L0 chunk #icx:10a2e4d3c2b1a098 (bytes 8192..14920)"]
    L1_1["L1 ToolArtifactNode #icx:7f3a91c0d2e4b6a8 (stub + line index)"]
    L1_2["L1 Turn #icx:8c4b02d17e9f3a65 (turn + AHTL packet)"]
    L2_1["L2 SubTrajectoryBranch #icx:b901c4e7a2d85f36 (folded debug/test loop)"]
    L3_1["L3 CacheEpoch #icx:e0013b7c9d2f4a58 (sealed epoch 0)"]

    L0_1 & L0_2 --> L1_1
    L1_1 & L1_2 --> L2_1
    L2_1 --> L3_1
```

### Node Types (`crates/icx-core/src/dag.rs`)

1. **`L0` chunks** live in the CAS (raw bytes plus BLAKE3 digest), not in the DAG enum.
2. **`DagNode::ToolArtifact` (`L1`):** tool name, agent, session, exit status, the CAS handle of the
   stored payload, ordered chunk ids, line-offset index, AHTL stub and surprise score, with a
   domain-separated Merkle digest (`ICX_DAG_TOOL_V1`).
3. **`DagNode::Turn` (`L1`):** ordinal, agent, role, CAS handle, AHTL representation and surprise
   score.
4. **`DagNode::SubTrajectoryBranch` (`L2`):** created when a sub-task is folded (Context-Folding,
   `arXiv:2510.11967`), for example through the MCP `fold` operation. Intermediate `L1` nodes are
   represented by one `⟦Φ:...⟧` commit packet pointing to its children.
5. **`DagNode::CacheEpoch` (`L3`/`L4`):** created when the unsealed window exceeds the rollover
   threshold. Seals a contiguous range of `L1`/`L2` nodes into an AHTL epoch frame with a BLAKE3
   Merkle root. Epoch headers write the epoch index in hexadecimal (`ep=000a` is epoch 10;
   `u32`, 4 to 8 hex digits; ICX-AUD-024).

---

## 4. Prefix-Cache Epoch Controller (`CacheEpochController`)

### 4.1 Zones and Configuration

The controller partitions the logical context into four zones:

| Zone | Mutability | Provider cache status (intended) |
| :--- | :--- | :--- |
| **Zone 0: Pinned system head** | Fixed per session | Stable prefix |
| **Zone 1: Sealed cache epochs (`E_0..E_{k-1}`)** | Cold history append-only; selected subset budgeted | Stable only while selected frames remain unchanged |
| **Zone 2: Active window (`W_active`)** | Append-only until rollover | Prefix-stable between rollovers |
| **Zone 3: Ephemeral tail slot** | Recomputed every turn | Not cached |

| Configuration (`EpochControllerConfig`) | Rollover tokens | Protected recent turns | Tail slot tokens |
| :--- | ---: | ---: | ---: |
| `Default` (used by the MMU) | 4,096 | 4 | 1,024 |
| `anthropic_default()` | 16,384 | 6 | 2,048 |
| `openai_default()` | 8,192 | 6 | 2,048 |

Whether a provider actually serves the stable prefix from cache depends on the provider, the
breakpoints the proxy or client places, and TTLs. `icx` measures only a byte-weighted
prefix-reuse proxy ([docs/performance.md](performance.md) §1).

### 4.2 Total Managed-Window Budget

`PromptWindowBudget` is persisted with the epoch controller. Its default is 32,768 total
accounting units, 4,096 completion reserve and 4,096 external-input reserve, leaving 24,576 bytes
for icx-managed head/history/tail. The identified estimator `utf8-bytes-upper-bound-v1` charges
one UTF-8 byte per token allowance; these are not counts from a provider tokenizer. The caller
must reserve actual system/current-message/tool-schema content outside the managed view and
choose the model's real limit. This does not automatically cap an entire native-agent or proxy
provider request containing external messages.

`assemble_window` includes separators in accounting and always keeps the newest contiguous run
of frames: all active turns, then the newest sealed epochs. When history no longer fits, the
oldest sealed epochs are evicted in one step down to half of the history allowance
(head and the maximum tail are reserved first), and that eviction point is persisted with the
controller. When half the allowance cannot be reached while keeping an epoch (one large epoch,
or a large active window), a step evicts only the fewest epochs that fit, never the whole
retained history (v1.2.1). Between eviction steps the retained prefix only grows by appending, so consecutive
windows share their prefix (0.94 byte reuse at saturation in the 8,192-unit regression test).
Omitted history is announced by a single frame,
`⟪ICX:HISTORY_OMITTED│epochs=N│turns=M│bytes=B│first=#icx:…│last=#icx:…│recall=icx:deref|slice|grep⟫`,
whose text changes only at eviction steps; the omitted epochs remain in DAG/CAS and stay
dereferenceable through those handles. No frame is skipped while an older one is kept.
An oversized pinned head fails closed to a bounded diagnostic rather than silently exceeding
the policy. `set_window_budget` refuses a policy that cannot hold the current pinned head.
The direct hook/proxy tail producer uses the same remaining allowance and the configured
tail-slot cap. It preserves the header, best recall and fitting complete bus/PPR frames; compact
CAS handles can replace oversized recoverable frames. Unsent sibling events remain queued and
do not advance the cursor. No partial packet or UTF-8 truncation is emitted.

Inspect or persist policy for an initialized/trusted workspace:

```bash
icx context-budget
icx context-budget --context-tokens 8192 --completion-reserve 1024 --external-reserve 2048
```

The latter leaves 5,120 bytes of managed allowance. It changes prompt selection, not CAS handles
or stored source bytes. Model switching can invalidate prefixes; a fixed provider cache-hit
rate is not promised. See [ADR 0006](adr/0006-v1-2-0-context-and-compatibility.md).

### 4.3 Epoch Rollover Algorithm

When `tokens(W_active) > epoch_rollover_tokens`:

1. Partition `W_active` into `[head_to_seal, protected_recent_tail]`, keeping the last
   `protect_last_n_turns` turns verbatim.
2. Fold resolved sub-trajectories inside `head_to_seal` into `L2` nodes.
3. Compile the remaining nodes into a sealed `L3` `CacheEpoch` (`E_k`), keeping full AHTL clauses
   for turns with surprise `>= 0.65` (`EPOCH_HIGH_SURPRISE_THRESHOLD`) and compact beacons for the
   rest.
4. Append `E_k` to Zone 1.

---

## 5. Workspace Persistence, Integrity & Backup

1. **Location and trust.** A workspace is a private directory outside the repository, by default
   `<state_dir>/workspaces/<first 16 hex of blake3(canonical project root)>`, where `state_dir` is
   `ICX_STATE_HOME`, else `$XDG_STATE_HOME/icx`, else `~/.local/state/icx`. `--dir` and `ICX_DIR`
   override it. The directory must be owned by the user, mode `0700`, and carry an `OWNER` seal;
   a repository-local `.icx/` is never used implicitly. See
   [docs/security-and-threat-model.md](security-and-threat-model.md) §4.1.
2. **Single SQLite store.** All state is in `icx.sqlite3` (WAL, mode `0600`): CAS chunk and
   artifact rows (append-only) plus `meta` rows for the chunk config, bi-temporal graph, MMU state
   (DAG, epochs and versioned window policy), HDC index snapshot (`ICXHDC05`), `generation` and store seal. Schema:
   [docs/data-model.md](data-model.md) §6.
3. **Atomic saves and acknowledgment.** Every save is one WAL transaction under the workspace
   lock (`cas_store.lock`) with `synchronous=FULL`. The daemon commits before acknowledging
   success; save failure yields an error response, the unsaved state is discarded and the daemon exits
   only after 5 consecutive failures. There is no volatile acknowledged queue. Atomicity prevents mixed-generation state, while durability still relies
   on filesystem/hardware synchronization behavior. The `generation` counter lets long-lived
   processes skip reloading when nothing changed.
4. **Fail-closed integrity.** On load, every chunk digest is verified and every artifact is
   reassembled to its full digest (`CasStore::from_parts`); the store seal (keyed BLAKE3 MAC over
   path, generation and a digest of all persisted state) must verify for sealed workspaces. Any
   failure stops the operation with an error instead of overwriting the store.
5. **Migration.** v1.0.x JSON workspaces (`cas_store.json`, `mmu_state.json`,
   `bitemporal_graph.json`) are migrated into the SQLite store on first load once the directory is
   trusted; the JSON files are left in place. Legacy HDC03 snapshots are readable, but the MMU
   rebuilds their absent exact postings from the CAS event log; subsequent saves write 04.
   Older binaries cannot read 04, so retain a pre-upgrade backup if downgrade is required.
6. **Backup.** Back up the whole workspace directory while no `icx` process is writing (stop
   `icx daemon` first). A copied workspace is refused at its new path until `icx trust --dir
   <path>` re-seals it, because the seal binds the canonical path.
7. **`icx doctor`.** Inspects agent installations and the bus path, loads the workspace and runs
   `CasStore::verify_all_integrity` and `MerkleContextDag::verify_all_merkle_integrity_with_cas`.

---

## 6. Edge Cases and Invariants

- **Model switch.** `set_window_budget` changes automatic selection without deleting `L0`
  content; `retarget_budget` remains an explicit API for hierarchical epoch folding. Budgeting
  does not require a test-only/manual retarget call to keep the managed view bounded.
- **Deterministic replay.** With the default logical clock, the same sequence of turns and tool
  outputs produces identical DAG digests and epoch frames. The CLI enables wall-clock timestamps
  (`use_wall_clock(true)`), and timestamps are part of node digests, so CLI workspaces are not
  bit-for-bit reproducible across runs.
