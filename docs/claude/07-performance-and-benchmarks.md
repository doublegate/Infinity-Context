# 07 — Performance and Benchmark Honesty

Workstream W7.

- **Host:** Intel i9-10850K (Comet Lake: AVX2 and POPCNT, **no AVX-512**), CachyOS, rustc 1.100.0.
- **Build:** release profile, with the host's global `-C target-cpu=native`. Numbers are
  therefore optimistic relative to default-target builds (see the POPCNT row in §2).
- **Runs:** `icx benchmark` executed 10 times.

## 1. Baseline: the shipped 9-SLO suite (10 runs)

| SLO | Median | Min–Max | CV | Documented targets/figures (sample) |
| --- | --- | --- | --- | --- |
| HDC bind + permute | 47.6 ns/op | 45.6–51.6 | 3.5% | — |
| HDC Hamming scan, 1,000 nodes | **72.7 µs** | 56.8–92.3 | 12.5% | `<25 µs` p99 (`performance.md:12`); 12.40 µs (deploy guide :1205); 1,306 µs (README :25) |
| Fractal suffix match | 22.6 ns/pair | 21.9–27.1 | 6.2% | — |
| Two-stage HDC + CSR PPR | 32.8 µs | 31.9–37.4 | 5.0% | — |
| AHTL compression ratio | 214.0× | fixed | 0% | 12–40×, 12–72×, 18.4–72.1×, 8–25× (various) |
| CAS ingest, 64 KiB | 89.5 µs | 86.3–92.4 | 2.2% | `<110 µs`; 64.5 µs (deploy guide) |
| Seqlock broadcast (in-memory only) | 62.9 ns | 57.1–66.0 | 4.0% | `<300 ns`, `<500 ns`, `<800 ns`, ~45 ns |
| Hook round-trip (in-memory MMU) | 272 µs | 263–301 | 4.0% | `<500 µs`; `<450 µs` p99; 118.4 µs (deploy guide) |
| 50-turn NIH + cache epoch | 95.38% | fixed | 0% | ≥94% |

All runs PASS. The suite's gates are loose: timing ceilings are multiplied by 100 in debug and by
3 in CI (`main.rs:939-946`). Each figure is a single pass over a synthetic fixture, with no
warm-up control and no percentiles. Three of the nine SLOs never touch the paths they are named
after: the hook SLO skips the disk reload, the CAS SLO produces a single chunk (ICX-AUD-027),
and the seqlock SLO uses only the in-memory bus.

## ICX-AUD-001 — Real hook latency grows linearly with history; 99% of it is re-indexing (Critical)

The shipped hook path is measured end to end: `icx hook --agent claude-code --event PostToolUse`
on a workspace built from realistic, non-repetitive `cargo test` outputs of about 40 lines.

| Artifacts in workspace | `.icx` size | Hook wall time (median of 5) |
| --- | --- | --- |
| ~14 | 1.2 MB | 35.8 ms |
| ~104 | 2.2 MB | 296 ms |
| 332 | 4.5 MB | ≈ 940 ms (by phase, below) |

Phase timing at 332 artifacts, library calls in process:

| Phase | Time |
| --- | --- |
| `load_workspace_dir_unlocked` | **935 ms** |
| `ClaudeCodeAdapter::handle_hook` | 0.18 ms |
| `save_workspace_dir_unlocked` | 6 ms |

`perf` profile of one hook (3,965 samples):

| Self time | Symbol |
| --- | --- |
| **87.5%** | `icx_hdc::hv::HdcBundler::add_delta` (scalar 10,240-counter loop, `hv.rs:638-690`) |
| 5.8% + 2.2% | BLAKE3 XOF (`Hypervector::from_seed`) |
| <1% | serde_json |

- **Cause:** every hook is a fresh process. `load_workspace_dir_unlocked` (`mmu.rs:1122-1290`)
  reconstructs the HDC index by *re-encoding every stored event*. Each token and bigram costs a
  10,240-iteration loop, so the cost is about **2.8 ms per stored artifact per hook**.
- **Impact:** at 1,000 stored tool outputs, roughly one long session, every tool call pays about
  3 s before the agent continues. Hooks run synchronously on every PostToolUse, so icx then
  slows the agent more than any context saving could repay. The documented "<500 µs hook" is not
  representative of the shipped binary; benchmark SLO 8 measures an in-memory MMU
  (`main.rs:1131-1155`).
- **Remediation (in order of payoff):**
  1. Persist the index itself: per-artifact hypervectors (1,280 B each), the entry summaries and
     the bundler state. Load becomes O(read), not O(re-encode).
  2. Replace the per-bit scalar loop with bit-sliced counters (carry-save adders over `u64`
     words), which is also the fix for ICX-AUD-050.
  3. Use one long-lived daemon per workspace (the proxy or MCP server process already exists)
     and turn hooks into thin RPC clients.
  4. Replace the three-JSON-file store with SQLite (WAL), which also fixes ICX-AUD-047.
- **Regression gate:** an end-to-end `icx hook` p99 under 50 ms with 10,000 stored artifacts,
  measured with the real binary on disk.

## ICX-AUD-002 — The ≥94% "prefix-cache hit rate" is set by padding, not by the epoch controller (High)

Benchmark 9 (`main.rs:1156-1215`) computes `prefix_cache_hit_ratio` (`epoch.rs:66-78`): the
length of the common *byte* prefix of two strings built in process, divided by the current
length. It is not a provider cache measurement.

- **What the benchmark does:**
  - It pins a head made of one 76-byte line **repeated 300 times** (22.8 KB of static prefix).
  - It ingests 50 near-identical payloads.
- **Correction to the pre-audit lead:** the filter `win.sealed_epochs.len() == prev_epochs` was
  thought to exclude seal turns. It is a no-op, because `prev_epochs` is sampled after ingest:
  49 of 49 turns are counted. The skew comes from the padding instead.

Re-running the exact benchmark with one variable changed at a time:

| Variant | Hit ratio | Epochs sealed |
| --- | --- | --- |
| As shipped (300× head, repetitive payloads) | 95.38% | 25 |
| 1× head, repetitive payloads | **69.92%** | 25 |
| 1× head, non-repetitive payloads | 81.85% | 48 of 49 turns |
| 300× head, non-repetitive payloads | 98.22% | 48 |

The figure moves with the size of the static padding, which shows that it reflects the fixture,
not the controller. The CLI gate also checks ≥ 90% (`main.rs:1208`), not the documented 94%.

- **Remediation:**
  - Drop the metric, or rename it "simulated byte-prefix stability".
  - Report real `cache_read_input_tokens / (cache_read + cache_creation + input)` from recorded
    provider responses on a replayed real session (see ICX-AUD-003).
  - Remove the padding and publish the turn-by-turn series.

## ICX-AUD-028 — Benchmark claims and published numbers are inconsistent (Medium)

- **Contradictory figures:** the same metric appears with figures 100× apart across documents
  (Hamming scan, table above). The measured median, 72.7 µs per 1,000 nodes, misses
  `performance.md`'s `<25 µs` p99 target.
- **AVX-512 timings:** "3.8 ns / 12 ns VPOPCNTDQ" (`docs/ahtl/04:188` and the PDF cover) cannot
  have been measured on the stated benchmark host, which has no AVX-512; `docs/ahtl/08:189`
  admits AVX2 + POPCNT.
- **Ingest stage sum:** the README ingest breakdown sums to 200 µs (25 + 60 + 15 + 85 + 15) but
  reports a total of 197.85 µs (`docs/ahtl/README.md:112-117`).
- **Inline-footprint ratio on real inputs** (`icx encode-ahtl`):

  | Input | Raw | Packet | Ratio |
| --- | --- | --- | --- |
  | this repository's `cargo test` log | 6,073 B | 110 B | 55.2× |
  | real `git diff 972698b c176d03 -- main.rs` | 28,539 B | 665 B | 42.9× |
  | `crates/icx-core/src/cas.rs` | 30,574 B | 888 B | 34.4× |

  These are within the documented 12–72× range. The benchmark's 214× fixture (1,199
  near-identical "ok" lines) is an outlier and should not be the headline. The ratio is a
  *skeleton + pointer* footprint, not information-preserving compression: the model sees the
  skeleton until it dereferences.
- **Remediation:**
  - Generate every published number from one benchmark run, stored as JSON and rendered into
    the docs.
  - Use `criterion` (or `divan`) with percentiles.
  - State the CPU and flags with each number.
  - Remove the AVX-512 timings, or label them as estimates.

## 2. Optimization opportunities (beyond ICX-AUD-001)

| Area | Observation | Suggestion |
| --- | --- | --- |
| `grep_cold` | Dereferences every artifact: O(total store) per query (`cas.rs:387-422`) | Trigram or line index built at ingest |
| Proxy concurrency | Every request holds one global MMU mutex plus the workspace lock while it loads, transforms and saves (`wire.rs:1965-2029`) | Shared in-memory MMU with an append-only log; per-request lock only around mutation |
| SSE parsing | Chunked SSE rescans from the start of the buffer, O(n²) (`wire.rs:1340-1356`) | Incremental line splitter |
| Persistence | Whole-store JSON rewrite on every hook; chunks serialized as JSON number arrays | Binary format or SQLite; raw chunk files |
| Hamming scan / POPCNT | The audit host's global `~/.cargo/config.toml` sets `-C target-cpu=native`, so the measured binary uses hardware POPCNT (`popcnt` appears inside `query_associative`). The crate itself has no `target_feature` or runtime dispatch, so CI builds and `cargo install` on default `x86-64` compile `count_ones` to a software popcount. Published numbers from a `native` build therefore overstate what users get | Add runtime dispatch (`is_x86_feature_detected!("popcnt")` with a `#[target_feature(enable = "popcnt")]` path; that needs `unsafe`, so use a safe wrapper crate or accept `x86-64-v2` as a documented baseline), and benchmark both the baseline and the native build in CI |
