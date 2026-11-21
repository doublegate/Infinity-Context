# Quantitative evidence and constants (v1.2.0 development)

Current implementation and qualification are tracked in
[the v1.2.0 release record](releases/v1.2.0/README.md). The tables headed v1.1.0 below are
**historical measurements**, not current release guarantees. The independent audit found that
their recall corpus was insufficient and their 10k scan figure was an arithmetic extrapolation.

The frozen 27-row v1.2.0 corpus rerun records recall@1/@5=100% across repetitive, varied and
multilingual synthetic cases at 100/1k/10k artifacts and three seeds. This establishes those
identifier-retrieval cases, not general semantic/model effectiveness. See the exact
[standard corpus](releases/v1.2.0/evidence/retrieval.csv) and
[multilingual corpus](releases/v1.2.0/evidence/retrieval-multilingual.csv).
Query timings in that run share the host with validation work and are not isolated SLO evidence.

Portable/native independent benchmark series and strict failures are retained in the release
record. Hardware SLO failures remain failures. Current `benchmark --json` measures actual 1k
and 10k scans independently and records build flags, CPU, corpus hash and gate outcomes.

This file is the single place where Infinity-Context (`icx`) states its quantitative claims.
Other documents should link here instead of restating a figure. The dated historical values
remain for comparison; current runtime contracts come from ADR 0006 and release evidence.
Cross-document contradictions recorded by
the audit (`ICX-AUD-031`) are listed with their canonical values in
[`ref-docs/corrections-2026-11-18.md`](../ref-docs/corrections-2026-11-18.md).

Every row is one of three kinds:

- **Benchmark:** output of `cargo run --release -p icx-cli -- benchmark` for v1.1.0. The suite is
  synthetic and in-process; its figures are not end-to-end agent latencies.
- **Measured:** a measurement taken during the v1.1.0 remediation on the development machine
  (Intel Core i9-10850K, AVX2, no AVX-512, Linux).
- **Constant:** a value fixed in source code or asserted by a test. It changes only when the code
  changes.

## 1. Benchmark Suite (`icx benchmark`)

Release-build gates apply as written; CI multiplies the time ceilings by 3 and debug builds by
100 (`crates/icx-cli/src/main.rs`, `scale`).

| Metric | v1.1.0 value | Fixture / how measured | Release gate | Source |
| :--- | :--- | :--- | :--- | :--- |
| HDC bind (`^`) + permute | 46.82 ns / op | 1,000 ops on 10,240-bit vectors | at most 100 ns | `icx benchmark` |
| HDC Hamming scan | 64.20 µs / 1,000 nodes; extrapolated 642.05 µs /10k | Historical 1k measurement multiplied by 10, not a measured 10k scan | at most 150µs per 1k | v1.1.0 `icx benchmark` |
| Fractal IFS suffix match | 24.27 ns / pair | `shared_depth=6` | at most 50 ns and depth at least 6 | `icx benchmark` |
| Two-stage HDC query | 49.00 µs | synthetic index | at most 200 µs | `icx benchmark` |
| CSR Personalized PageRank | 109.78 µs | 201-node graph, 8 iterations | at most 400 µs | `icx benchmark` |
| AHTL inline ratio, repetitive log | 214.0x (61,413 B to 287 B) in 44.43 µs | repetitive 1,200-line test log | ratio at least 10x, time at most 400 µs | `icx benchmark` |
| AHTL inline ratio, varied log | 159.8x (14,700 B to 92 B) | varied log; informational, not gated | none | `icx benchmark` |
| FastCDC + BLAKE3 CAS ingest | 86.72 µs | 64,659 B, 8 chunks | at most 250 µs | `icx benchmark` |
| In-process seqlock publish | 58.3 ns / slot | 1,000 publishes, in-memory ring | at most 5,000 ns | `icx benchmark` |
| Synchronous hook round trip | 244.05 µs | one Claude Code `PostToolUse` on a fresh in-memory MMU (no persisted workspace) | at most 1,000 µs | `icx benchmark` |
| Byte-weighted prefix reuse, all turns | 95.46% | synthetic 50-turn transcript, seal turns included; a byte proxy, not a provider cache hit rate | at least 90% (`PREFIX_REUSE_FLOOR_PCT`) | `icx benchmark` |
| Byte-weighted prefix reuse, excluding seal turns | 95.38% | same transcript | none | `icx benchmark` |
| Passkey recovery in the 50-turn run | recovered (18.13 ms for the run) | planted passkey, recovered by `deref` and cold grep | must be recovered | `icx benchmark` |

The AHTL ratio is the size of a skeleton plus a handle relative to the input. It is not a
lossless encoding: the model sees the skeleton until it dereferences the handle, and the full
text comes from `deref`.

## 2. Measured Outside the Benchmark

| Metric | v1.0.0 | v1.1.0 | How measured | Source |
| :--- | :--- | :--- | :--- | :--- |
| `icx hook`, in-process, 338-artifact workspace | 940 ms | 14 ms | real workspace, one hook invocation | audit remediation measurement |
| `icx hook`, in-process, about 2,000 artifacts | not measured | 69–72 ms | real workspace | audit remediation measurement |
| `icx hook` via `icx daemon`, about 2,000 artifacts | not available | median 3.1 ms; p90 3.4 ms in a burst; about 19 ms when a request overlaps a flush | resident daemon over its Unix socket | audit remediation measurement |
| HDC unique-token recall, top-1 | 5.1% | 99.0% | 1,000 log documents | audit remediation measurement |
| HDC unique-token recall, top-5 | 5.6% | 99.7% | 1,000 log documents | audit remediation measurement |
| HDC query time on the recall corpus | 24 µs | 69 µs | same corpus | audit remediation measurement |
| File-backed bus, publish to visible | not measured | worst 38 µs over 200 events | two handles on one file | audit remediation measurement |
| FastCDC mean chunk size (8 KiB target) | 13.2 KiB | 10.6 KiB | spread masks (Xia et al., ATC'16) | audit remediation measurement |

## 3. Constants and Test-Asserted Bounds

### Handles, hashing and storage

| Quantity | Value | Source |
| :--- | :--- | :--- |
| Handle widths | 64-bit (`#icx:<16-hex>`) and 48-bit (`#icx:<12-hex>`) prefixes of the BLAKE3 digest | `icx deref` help; `crates/icx-core` |
| 50% collision point, 48-bit prefix (derived) | about 1.98 × 10^7 artifacts (1.1774 · 2^24) | birthday bound, `N ≈ sqrt(2 ln 2 · 2^48)` |
| 50% collision point, 64-bit prefix (derived) | about 5.06 × 10^9 artifacts (1.1774 · 2^32) | birthday bound, `N ≈ sqrt(2 ln 2 · 2^64)` |
| Behaviour on a 64-bit handle collision | `IcxError::HashCollision`; payload not virtualized, nothing overwritten | `crates/icx-core` |
| Maximum ingest size | 64 MiB (`ICX_MAX_INGEST_BYTES`) | `crates/icx-core/src/lib.rs` |
| Maximum hook stdin | 64 MiB + 64 KiB (`MAX_STDIN_BYTES`) | `crates/icx-cli/src/main.rs` |
| Store layout | one SQLite file `icx.sqlite3` (WAL) per workspace | `crates/icx-cli` |
| Workspace permissions | directory `0700`, files `0600`; install key `0600`, 32 random bytes | `crates/icx-cli` |
| Default workspace id | first 16 hex digits of `blake3(canonical project root)` | `crates/icx-cli` |

### Codec (`icx-codec`)

| Quantity | Value | Source |
| :--- | :--- | :--- |
| Maximum clause chain | 64 `⇒` links (`MAX_CLAUSE_CHAIN`), parsed iteratively | `crates/icx-codec/src/ahtl.rs:90` |
| Epoch header id | hex `u32`, 4 to 8 hex digits (`ep=000a` is 10) | `crates/icx-codec/src/ahtl.rs` |
| Braille-256 sketch sizes | 8 bytes (Profile B) and 64 bytes (Profile A), one byte per glyph | `crates/icx-codec/src/braille.rs` |
| Redactor tiers | 4 (Tier 0 delimiter escaping, Tier 1 key blocks, Tier 2 prefixes and assignments, Tier 3 entropy) | `crates/icx-codec/src/redact.rs` |
| Entropy gate | Shannon character entropy above 4.35 bits/char | `crates/icx-codec/src/redact.rs:863` |
| `FLAG_IS_REDACTED` | bit `1 << 7` of `ChannelBHologram` byte `B[7]` | `crates/icx-codec/src/braille.rs` |

### Hyperdimensional index and graph (`icx-hdc`, `icx-store`)

| Quantity | Value | Source |
| :--- | :--- | :--- |
| Hypervector width | 10,240 bits (`HV_BITS`), 160 `u64` words | `crates/icx-hdc/src/hv.rs:8` |
| Fractal coordinate in memory | `[f64; 16]` | `crates/icx-hdc/src/fractal.rs` |
| Recall gate test | recall@5 at least 95% at n = 300 | `crates/icx-hdc/tests/recall_v110.rs` |
| HDC snapshot format | `ICXHDC05` (binary, v1.2.2); 04 and 03 readable, postings rebuilt from CAS events by MMU | `crates/icx-hdc/src/snapshot.rs` |
| PPR restart parameter | 0.15 | `crates/icx-adapters/src/mmu.rs:848` |
| PPR iterations, tail-slot recall | 6 | `crates/icx-adapters/src/mmu.rs:848` |
| PPR iterations, benchmark | 8 | `crates/icx-cli/src/main.rs:1429` |

### Epoch controller and proxy (`icx-core`, `icx-proxy`)

| Quantity | Value | Source |
| :--- | :--- | :--- |
| `EpochControllerConfig::default()` (used by the MMU) | 4,096 rollover tokens, 4 protected turns, 1,024-token tail slot | `crates/icx-core/src/epoch.rs:21-29` |
| `anthropic_default()` preset | 16,384 / 6 / 2,048 | `crates/icx-core/src/epoch.rs` |
| `openai_default()` preset | 8,192 / 6 / 2,048 | `crates/icx-core/src/epoch.rs` |
| Anthropic breakpoints | client breakpoints kept; icx adds at most 1, only while the total is below 4 | `crates/icx-proxy/src/wire.rs:109-148` |
| Messages left unfolded at the tail | last 2 | `crates/icx-proxy/src/wire.rs:99` |
| Default proxy listen address | `127.0.0.1:49380` | `icx --help` |
| Maximum proxied request body | 16 MiB | `crates/icx-proxy/src/wire.rs:1173` |

### Bus (`icx-bus`)

| Quantity | Value | Source |
| :--- | :--- | :--- |
| Ring slots | 256 (`ICX_RING_SLOTS`) | `crates/icx-bus/src/ring.rs:15` |
| Slot size | 4,096 bytes | `crates/icx-bus/src/ring.rs:20` |
| Inline AHTL capacity per slot | 3,968 bytes (`ICX_AHTL_INLINE_CAP`) | `crates/icx-bus/src/ring.rs:16` |
| Fabric header | 64 bytes | `crates/icx-bus/src/ring.rs:19` |
| Writer spin limit | 4,096 iterations (`MAX_WRITER_SPIN_ITERS`) | `crates/icx-bus/src/ring.rs:24` |
| File-backed transport | `pread`/`pwrite` under a writer mutex and file lock; not memory-mapped, not lock-free, not zero-copy | `crates/icx-bus/src/ring.rs` |

### Adapters, MCP and daemon (`icx-adapters`, `icx-cli`)

| Quantity | Value | Source |
| :--- | :--- | :--- |
| Claude Code PreCompact limit (upstream) | fewer than 10,000 characters | Claude Code hooks documentation |
| icx PreCompact cap | 9,000 characters (`MAX_COMPACTION_INSTRUCTION_CHARS`) | `crates/icx-adapters/src/claude.rs:199` |
| Maximum fold depth | 64 (`MAX_FOLD_DEPTH`) | `crates/icx-adapters/src/mcp.rs:17` |
| Maximum MCP input line | 4 MiB | `crates/icx-cli/src/main.rs:90` |
| MCP tool schema size | under 180 tokens by a characters/4 estimate | test in `crates/icx-adapters/src/lib.rs:2043` |
| MCP protocol versions | legacy `initialize`: 2025-11-25, 2025-06-18, 2025-03-26, 2024-11-05; modern: 2026-07-28 | `crates/icx-adapters/src/mcp.rs` |
| agy hook timeout default (upstream) | 30 s | Antigravity hooks documentation |
| Daemon acknowledgment | Success only after SQLite WAL FULL commit under workspace lock; save errors propagate | `crates/icx-cli/src/daemon.rs`; `store.rs` |
| Unix socket path limit | 103 bytes (longer paths are rejected) | `crates/icx-cli/src/daemon.rs:108` |

### Project and quality gates

| Quantity | Value | Source |
| :--- | :--- | :--- |
| Workspace crates | 8 | `Cargo.toml` |
| Line coverage gate | at least 95% | `.github/workflows/ci.yml:105` |
| Fuzz targets | 4 (`ahtl_stream`, `hdc_snapshot`, `mcp_jsonrpc`, `redact`) | `fuzz/fuzz_targets/` |
| Roadmap sprints | 15 | `to-dos/phase-*/sprint-*.md` |
| arXiv papers indexed | 22 | `ref-docs/papers/INDEX.md` |
| arXiv PDFs redistributed | 12 (11 CC BY 4.0, 1 CC BY-NC-SA 4.0) | `ref-docs/papers/` |

## 4. Figures That Are Not Claimed

The following appeared in earlier documents and are not claims of v1.1.0. Do not reintroduce
them without a measurement:

- any "cache hit rate" presented as a provider-measured figure (the 94.38% and 94%-99% figures of
  v1.0.0 came from an earlier simulation; the v1.1.0 metric is the byte proxy in section 1);
- `< 300 ns`, `< 500 ns`, `< 800 ns` or `< 1 µs` file-backed or `/dev/shm` publish latencies;
- AVX-512 / `VPOPCNTDQ` timings, which cannot have been measured on the AVX2-only benchmark host;
- hook latencies of `< 500 µs` p99 or similar for a real workspace (see section 2);
- compression ranges such as 12–72x stated without the input they were measured on;
- figures attributed to research papers that the papers do not contain (see the corrections file).
