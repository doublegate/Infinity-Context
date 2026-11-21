# Performance Budgets, Hot-Path Engineering & Profiling Plan

**Applies to:** `icx` v1.2.0 development; historical tables below explicitly retain v1.1.0 results.
**References:** [ref-docs/initial-concept.md](../ref-docs/initial-concept.md) §4, [docs/architecture.md](architecture.md) §3

The current [release record](releases/v1.2.0/README.md) links raw v1.2.0 results and failed gates.
Historical numbers below came from the development machine (Intel i9-10850K, Linux; no
AVX-512), except the old10k scan arithmetic extrapolation. They are not current guarantees.
The new JSON benchmark measures1k/10k independently. Its local ceilings remain explicit;
both original and optimized portable/native series fail them. Ceilings remain unchanged.

The optimized series used one excluded warm-up and10measured processes per build, after
this task's heavy jobs stopped. All10processes in each build failed at least one strict gate:

| Current median | Portable x86-64 | Native | Local ceiling |
| --- | ---: | ---: | ---: |
| Bind + permute | 144.27ns | 82.87ns | 100ns |
| Fresh independent1k scan | 121.06µs | 530.95µs | 150µs |
| Fresh independent10k scan | 2784.33µs | 9099.64µs | 1500µs |

See [raw series/provenance](releases/v1.2.0/evidence/benchmark-quiet-v2-summary.json) and
[diagnosis/limits](releases/v1.2.0/evidence/benchmark-v2-diagnosis.json). Unrelated host LTO
compilation was subsequently observed; this was a coordinated task window, not an isolated
idle host. CPU-affinity diagnostics remained unstable. Contention is established, but its
full causal contribution is not. Repeat qualification under declared stable workload before
claiming performance readiness; the matched kernel improvement does not replace this gate.

---

## 1. `icx benchmark` SLO Gates

`icx benchmark` (`run_benchmark` in `crates/icx-cli/src/main.rs`) is a **synthetic, in-process**
suite. Its numbers measure individual components inside one process; they are not end-to-end
agent latencies and include no process start-up, store I/O or agent overhead. Release-build
ceilings are multiplied by 3 in CI and by 100 in debug builds.

| Operation | Gate (release, local) | Measured v1.1.0 |
| :--- | ---: | ---: |
| HDC 10,240-bit bind (`^`) + permute | `<= 100 ns` | 46.82 ns / op |
| HDC Hamming scan | `<=150µs/1k`, `<=1500µs/10k`, independently measured in1.2 | Historical64.20µs/1k;642.05µs/10k was extrapolated |
| Fractal IFS suffix match | `<= 50 ns`, shared depth `>= 6` | 24.27 ns / pair |
| Two-stage HDC query / 201-node CSR PPR | `<= 200 µs` / `<= 400 µs` | 49.00 µs / 109.78 µs |
| AHTL compression on a repetitive 1,200-line test log | ratio `>= 10x`, `<= 400 µs` | 214.0x (61,413 B to 287 B) in 44.43 µs |
| AHTL ratio on a varied log (informational, not gated) | none | 159.8x (14,700 B to 92 B) |
| FastCDC + BLAKE3 CAS ingest, 64 KiB | `<= 250 µs` | 86.72 µs |
| Telepathy ring publish, in-process backend | `<= 5 µs` | 58.3 ns / slot |
| Hook round-trip (`PostToolUse` virtualization, in-process MMU) | `<= 1,000 µs` | 244.05 µs |
| 50-turn synthetic transcript: byte-weighted prefix reuse | `>= 90%` | 95.46% over all turns (95.38% excluding seal turns) |

**Compression ratio.** An AHTL packet is a structural skeleton plus a `#icx:` handle, not a
lossless encoding of the input. The full text is recovered by dereferencing the handle. The 214x
figure is on a deliberately repetitive fixture; it is not a general token-reduction rate.

**Prefix reuse.** The cache line is a **byte-weighted prefix-reuse proxy**: the share of prompt
bytes identical to the previous turn's prefix, over all turns of a synthetic transcript,
including turns that seal an epoch. It is not a measured provider cache hit rate (ICX-AUD-002).
Real provider cache behaviour depends on breakpoint placement, TTLs and provider policy.

---

## 2. Measured End-to-End Hook Cost

v1.0.0 reloaded and re-encoded the whole workspace on every hook, so hook cost grew with
workspace size. v1.1.0 loads one SQLite store with a persisted HDC snapshot, and the resident
daemon avoids the load entirely.

| Scenario | v1.0.0 | v1.1.0 |
| :--- | ---: | ---: |
| `icx hook`, 338 artifacts, in-process | 940 ms | 14 ms |
| `icx hook`, about 2,000 artifacts, in-process | not measured | 69 to 72 ms |
| `icx hook` via the resident daemon, about 2,000 artifacts | n/a | median 3.1 ms; p90 3.4 ms in a burst; about 19 ms when a request overlaps a flush |

---

## 3. Other Measurements

| Item | Result |
| :--- | :--- |
| HDC unique-token recall, 1,000 log documents | top-1 5.1% to 99.0%, top-5 5.6% to 99.7% (v1.0.0 to v1.1.0); query 24 µs to 69 µs. Gate test: recall@5 `>= 95%` at n = 300 (`crates/icx-hdc/tests/recall_v110.rs`). |
| File-backed telepathy ring, publish to visible in a second handle | worst case 38 µs over 200 events (`crates/icx-bus/tests/loss_signalling.rs`) |
| FastCDC mean chunk size on this repository's sources (target 8 KiB) | 13.2 KiB to 10.6 KiB after switching to spread masks |

---

## 4. Hot-Path Engineering Rules

1. **No heap allocation in HDC and fractal inner loops.** `Hypervector` (`[u64; 160]`) and
   `FractalCoord` (`[f64; 16]`) are fixed-size `Copy` values; `Hypervector` is
   `#[repr(C, align(64))]`. Hamming distance uses `u64::count_ones()` over the fixed array
   (`POPCNT` on x86-64; wider vector instructions only where the compiler and target CPU provide
   them).
2. **Single-pass structural skeletonization.** `StructuralSkeletonizer` scans input once by line
   without building ASTs or running backtracking regular expressions.
3. **Telepathy ring.** The in-process backend uses atomics only. The file-backed backend uses
   positional I/O under a writer mutex and an advisory file lock; it is not memory-mapped, not
   lock-free for writers and not zero-copy (see
   [docs/wire-protocol-and-shm-bus.md](wire-protocol-and-shm-bus.md)).
4. **Hybrid candidates.** Exact postings supply all token matches with lexical-first ranking.
   Common tokens can therefore require full refinement; there is no unconditional bounded
   candidate count. Stage1 additionally scores entries on a1,024-bit
   hypervector prefix, a 256-bit content-token Bloom filter and summary fields; Stage 2 runs the
   full 10,240-bit comparison on at least `4 * sqrt(n)` candidates.
5. **Amortized epoch sealing.** `CacheEpochController` seals an epoch only when the unsealed
   window exceeds its rollover threshold (4,096 tokens by default; presets 16,384 for Anthropic
   and 8,192 for OpenAI).
6. **Committed daemon receipts.** The daemon saves before success, under the workspace lock.
   SQLite WAL FULL synchronizes commits. Real socket/store latency must be measured separately
   from the in-memory hook microbenchmark; the old write-behind latency is historical.

---

## 5. Profiling & Benchmarking Workflow

- Run the benchmark suite:

  ```bash
  cargo run --release -p icx-cli -- benchmark
  ```

- Hardware performance counters on Linux:

  ```bash
  perf stat -e cycles,instructions,cache-references,cache-misses,branches,branch-misses \
    cargo test --release --workspace
  ```

- Record before and after numbers for any performance change (Module 30).
