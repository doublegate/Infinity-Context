# Performance Budgets, Hot-Path Engineering & Profiling Plan

**References:** [ref-docs/initial-concept.md](../ref-docs/initial-concept.md) §4, [docs/architecture.md](architecture.md) §3

---

## 1. Latency & Throughput Service-Level Objectives (SLOs)

| Operation | Hot-Path Target (p99) | Hard Ceiling | Verification Benchmark |
| :--- | ---: | ---: | :--- |
| **HDC 10,240-bit Bind (`^`) + Permute** | `< 25 ns` | `100 ns` | `benches/hdc_bench.rs` |
| **HDC Hamming Scan (10,000 nodes)** | `< 25 µs` | `100 µs` | `benches/hdc_bench.rs` |
| **Fractal IFS Trajectory Suffix Match** | `< 15 ns` | `50 ns` | `benches/fractal_bench.rs` |
| **AHTL Structural Skeletonize + Encode (64 KB log)** | `< 180 µs` | `400 µs` | `benches/codec_bench.rs` |
| **FastCDC + BLAKE3 CAS Ingest (64 KB)** | `< 110 µs` | `250 µs` | `benches/cas_bench.rs` |
| **`/dev/shm` Seqlock Ring Publish / Poll** | `< 800 ns` | `5 µs` | `benches/bus_bench.rs` |
| **Full Synchronous Hook Round-Trip (`icx hook`)** | `< 450 µs` | `1,000 µs` | `icx benchmark` |
| **In-Window Token Compression Ratio** | `15x–40x` | `>= 10x` | `icx benchmark` |
| **Provider Prompt Prefix-Cache Hit Ratio** | `>= 94%` | `>= 90%` | `tests/cache_epoch_sim.rs` |

---

## 2. Hot-Path Engineering Rules

1. **Zero Heap Allocations in HDC & Fractal Inner Loops:**
   - `Hypervector` (`[u64; 160]`) and `FractalCoord` (`[f64; 16]`) are stack-allocated `Copy` structs aligned to 64-byte cache lines (`#[repr(C, align(64))]`).
   - Hamming distance uses hardware `u64::count_ones()` (`POPCNT` / AVX-512 `VPOPCNTDQ`), which LLVM auto-vectorizes across the 160-element fixed array into `zmm` register loops.
2. **Single-Pass Structural Skeletonization:**
   - `StructuralSkeletonizer` scans input strings once by line slice without constructing intermediate AST heap trees or running backtracking regular expressions.
3. **Lock-Free Shared Memory:**
   - `SeqlockRing` uses `AtomicU64` sequence counters (`Acquire`/`Release` memory ordering) over a fixed `4080`-slot array—zero syscalls, zero mutexes, zero allocations after `mmap`.
4. **Amortized Epoch Sealing:**
   - `CacheEpochController` seals epochs only when `unsealed_tokens > epoch_rollover_threshold` (`16,384` tokens), keeping `98%+` of turns append-only.

---

## 3. Profiling & Benchmarking Workflow

- Run built-in latency and compression benchmarks:
  ```bash
  cargo run --release -p icx-cli -- benchmark
  ```
- CPU hardware performance counter profiling on Linux (i9-10850K):
  ```bash
  perf stat -e cycles,instructions,cache-references,cache-misses,branches,branch-misses \
    cargo test --release --workspace
  ```
