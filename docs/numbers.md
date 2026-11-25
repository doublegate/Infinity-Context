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

**v1.4.0 measurements (2026-11-22, release build, desktop):**

| Metric | Value | How measured | Source |
| :--- | :--- | :--- | :--- |
| One stub-mode OpenCode `execute.after` hook, 28 MB store | 0.15 s in-process; 0.07 s through a warm daemon | store from one project's Codex history | Sprint 4 notes (#43) |
| Same hook, 825 MB store | 4.4–4.6 s in-process (1.7 GB peak); 4.2 s first daemon request, about 2.0 s warm | store from all projects' Codex history, tool outputs included | Sprint 4 notes (#43) |
| Six parallel Claude Code `PostToolUse` hooks, small store | all done 0.023–0.087 s after start; `PostToolBatch` 0.014 s | `claude -p`, six parallel Bash calls, timing wrapper around `icx hook` | T-64-004 |
| `icx codex-ingest --all-projects`, tool outputs included | 523 s with batched lock release; 426 s unbatched | 574 rollouts (518 compressed) | Sprint 2 notes |

Hooks of one agent take the workspace lock in turn, so a batch of N parallel tool calls costs
about N times one hook. On a small store that is milliseconds; at about 2 s per hook (825 MB), the
last of six would wait about 12 s. The cost grows with store size until saves become incremental
(Phase 9).

**v1.5.0 measurements: stage-1 prescreen (2026-11-24, release build, i9-10850K, T-71-003):**

Synthetic corpora of n events (24 words each from a 50,000- or 500-word vocabulary), 200 queries
of three distinct words of the target. Stage-1 recall is the share of queries whose target is in
the candidate set (prescreen survivors plus the postings union) that exact scoring sees; the query
time is `query_associative` alone. Both widths are measured by one command:
`cargo test --release -p icx-hdc --test measurements measure_stage1 -- --ignored --nocapture`.

| prescreen | n | vocabulary | queries | max candidates | artifacts with all 3 words (mean) | stage-1 recall | recall@1 | recall@5 | mean query |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1024 bits | 1000 | 50000 | 200 | 128 | 1.0 | 100.0% | 100.0% | 100.0% | 0.17 ms |
| 1024 bits | 1000 | 500 | 200 | 219 | 1.1 | 100.0% | 95.0% | 100.0% | 0.20 ms |
| 1024 bits | 10000 | 50000 | 200 | 413 | 1.0 | 100.0% | 100.0% | 100.0% | 1.12 ms |
| 1024 bits | 10000 | 500 | 200 | 714 | 1.9 | 100.0% | 69.0% | 100.0% | 1.08 ms |
| 1024 bits | 100000 | 50000 | 200 | 1403 | 1.0 | 100.0% | 100.0% | 100.0% | 15.63 ms |
| 1024 bits | 100000 | 500 | 200 | 1265 | 10.1 | 100.0% | 15.0% | 55.5% | 15.24 ms |
| 2048 bits | 1000 | 50000 | 200 | 128 | 1.0 | 100.0% | 100.0% | 100.0% | 0.17 ms |
| 2048 bits | 1000 | 500 | 200 | 219 | 1.1 | 100.0% | 95.0% | 100.0% | 0.21 ms |
| 2048 bits | 10000 | 50000 | 200 | 412 | 1.0 | 100.0% | 100.0% | 100.0% | 1.20 ms |
| 2048 bits | 10000 | 500 | 200 | 713 | 1.9 | 100.0% | 69.0% | 100.0% | 1.09 ms |
| 2048 bits | 100000 | 50000 | 200 | 1402 | 1.0 | 100.0% | 100.0% | 100.0% | 26.75 ms |
| 2048 bits | 100000 | 500 | 200 | 1265 | 10.1 | 100.0% | 15.0% | 55.5% | 26.80 ms |

- **1,024-bit prescreen (shipped since v1.1.0):** the target reaches exact scoring in every
  query, at every size, for rare and common words alike.
- **2,048 bits** (its score scaled to the 1,024-bit range, so only the width differs): identical
  recall in every row, and about 1.7 times slower at 100k (26.8 ms against 15.6 ms; the
  measurement run alone, since timings on a shared desktop vary by tens of percent). The hypothesis that a wider
  prescreen recovers dropped candidates is rejected, and the width stays 1,024 bits.
- **The loss with common words is ranking, not the prescreen:** at 100k, about 10 artifacts hold
  all three query words, so recall@1 cannot be much above 1/10 by these words alone; 15.0% is
  above that. Lexical ranking (BM25, T-71-001) and fusion (T-71-002) are measured against this
  on the Sprint 2 evaluation set.
- **Query time** grows with n: about 0.2 ms at 1k, 1.1 ms at 10k and 15–16 ms at 100k.
- **Corrections (review of #97):** a first version timed the stage-1 inspection and the
  ambiguity count with the query, sampled query words with replacement (so some queries had two
  distinct words), and did not scale the 2,048-bit score; it reported about twice these times and
  lower recall for common words.

**ICX-AUD-052 measurement (T-71-004):** over 20,000 events with no repeats, the coordinates alone
claimed a shared history for 3 distinct and 66 similar earlier events; the confirmed suffix
claims none. Reproduce with
`cargo test --release -p icx-hdc --test measurements measure_fractal -- --ignored --nocapture`.

**v1.5.0 measurements: lexical index (2026-11-24, release build, i9-10850K, SQLite 3.54.1,
T-71-001):**

The production code (`lexical::insert`, `search_words`, `trigram_candidates`) on the text of a real
54 MB workspace store: 5,230 artifacts, 8.5 MB of text, indexed in one transaction as a backfill
does, without `optimize` or `VACUUM`. Reproduce on any store with
`ICX_MEASURE_STORE=<path/to/icx.sqlite3> cargo test --release -p icx-adapters --test lexical_index measure -- --ignored --nocapture`
(the store is opened read-only; only sizes and timings are printed).

| Measure | Value |
| :--- | :--- |
| Index size (`lex_ids`, `lex_words`, `lex_trigrams`) | 10.2 MB: 1.2 times the text, 19% added to this store |
| Insert, identifier parts included | 0.26 ms per artifact |
| BM25 top 20, 2–4 query terms | 0.28–1.19 ms median, 1.54 ms worst |
| Trigram candidates, 5–9 character needle | 0.31–0.56 ms median, 1.24 ms worst |

- **Compact row numbers matter:** with the artifact id (a random 64-bit number) as the FTS rowid
  the same index took 38.5 MB, because FTS5 stores rowid gaps as varints and every posting cost
  about 9 bytes. Rows are therefore numbered compactly and mapped through `lex_ids`.
- **Corrections (review of #97):** the first figures (3.0 and 3.7 MB) came from a Python probe that
  numbered rows 0, 1, 2…, left the identifier-parts column empty and compacted the tables, so they
  did not describe the shipped index.
- These timings reuse one open connection. Through the MMU (`lexical_matches`,
  `substring_matches`) each call also opens the store read-only and checks the table version, and
  a substring hit reads its candidates from the CAS until `limit` are confirmed (at most
  `max(64, 8 * limit)`).

**v1.5.0 measurements: recall evaluation (2026-11-24, `icx eval-recall`, T-72-001..004):**

Real-session cases: a `deref` or `slice` of a handle the agent got from a stub; the query is the
agent's note before the call or the user's prompt that started the turn; each query ranks only
artifacts that existed when it was asked. Queries and judgements are private
(`eval/private/`, built by `scripts/build-recall-eval-set.py`); these are aggregates.

| Agent notes (this project, held-out half, 372 queries) | legacy | BM25 | RRF | newest first |
| :--- | ---: | ---: | ---: | ---: |
| recall@1 | 0.0% | 2.2% | 0.3% | 83.3% |
| recall@10 | 2.4% | 12.6% | 11.6% | 99.2% |

- **RRF is the shipped fusion** (`InfinityContextMmu::fused_ranking`: the first 4 displayable HDC
  hits with the first 20 BM25 matches). A first version fused 50 unfiltered HDC hits and reported
  5.4% recall@10 (corrected in the review of #98).
- **Fresh answers dominate:** across all six projects with at least 40 dereferences (1,676 note
  cases), 7 had 20 or more newer artifacts between the answer and the query. The table therefore
  measures fetching what was just produced, not long-range recall, and cannot decide a default
  (ADR-0010).
- **Prompts as queries:** timed at the prompt, when automatic recall runs, 313 of 316 prompt
  cases had answers that did not exist yet (the agent produced them during the turn). Only 3
  cases could have been answered by recall at all, too few to score.
- **Every indexed artifact was dated** from a graph edge in each workspace measured; an undated
  one would take the next dated time (an upper bound, so the cutoff fails closed).
- **Synthetic sets** (`icx eval-recall --synthetic <n> --vocab <v>`, seed 7, 200 queries): with a
  50,000-word vocabulary every ranker has recall@1 of 100% at 1k and 10k artifacts; with 500
  words at 10k, recall@1 is 59.0% (legacy), 56.5% (BM25) and 58.5% (RRF).

**v1.5.0 measurements: re-reads after compaction (2026-11-25, T-73-002 baseline):**

Across this machine's Claude Code transcripts with compactions (20 transcripts, 336 compactions),
of the 30 tool calls after each compaction, 423 of 9,657 (4.4%) repeated a call made before it
(the same file read, search, command or icx handle). Some repeats are deliberate (re-running
tests). This is the "without packet" baseline; the "with packet" figure needs sessions recorded
after v1.5.0 is installed, and is not claimed until then.

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
| `icx rescan` on a field workspace (v1.5.0) | 4,694 artifacts in 1.9 s, 75 MB peak RSS: 169 affected (stored by versions before v1.3.1), 76 markers, 5 subjects; the store unchanged | `icx rescan --limit 0` on this project's workspace, 2026-11-25 |
| Structural redactor corpus (v1.5.0, ADR-0011) | 19 lines, 590 bytes: missed secrets 0 before and after; bytes hidden that were not a secret 183 before (v1.4.1 rule), 24 after | `crates/icx-codec/tests/redact_structural_v150.rs` (`measure_corpus`, ignored; before measured on `main` at 80f4b94) |
| `FLAG_IS_REDACTED` | bit `1 << 7` of `ChannelBHologram` byte `B[7]` | `crates/icx-codec/src/braille.rs` |

### Hyperdimensional index and graph (`icx-hdc`, `icx-store`)

| Quantity | Value | Source |
| :--- | :--- | :--- |
| Hypervector width | 10,240 bits (`HV_BITS`), 160 `u64` words | `crates/icx-hdc/src/hv.rs:8` |
| Fractal coordinate in memory | `[f64; 16]` | `crates/icx-hdc/src/fractal.rs` |
| Recall gate test | recall@5 at least 95% at n = 300 | `crates/icx-hdc/tests/recall_gate.rs` |
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
