# Performance Budgets, Hot-Path Engineering & Profiling Plan

**References:** [ref-docs/initial-concept.md](../ref-docs/initial-concept.md) §4, [docs/architecture.md](architecture.md) §3

---

## 1. Latency & Throughput Service-Level Objectives (SLOs)

| Operation | Hot-Path Target (p99) | Hard Ceiling | Verification Benchmark |
| :--- | ---: | ---: | :--- |
| **HDC 10,240-bit Bind (`^`) + Permute** | `< 25 ns` | `100 ns` | `icx benchmark` |
| **HDC Hamming Scan (1,000 nodes)** | `< 25 µs` | `150 µs` | `icx benchmark` |
| **Fractal IFS Trajectory Suffix Match** | `< 15 ns` | `50 ns` | `icx benchmark` |
| **AHTL Structural Skeletonize + Encode (64 KB log)** | `< 180 µs` | `400 µs` | `icx benchmark` |
| **FastCDC + BLAKE3 CAS Ingest (64 KB)** | `< 110 µs` | `250 µs` | `icx benchmark` |
| **`SeqlockTelemetryBus` Publish / Poll (`InProcess` atomic / `/dev/shm` `SharedFile`)** | `< 800 ns` (`InProcess`) / `< 15 µs` (`SharedFile`) | `5 µs` / `100 µs` | `icx benchmark` |
| **Full Synchronous Hook Round-Trip (`icx hook`)** | `< 450 µs` | `1,000 µs` | `icx benchmark` |
| **In-Window Token Compression Ratio** | `15x–40x` | `>= 10x` | `icx benchmark` |
| **Provider Prompt Prefix-Cache Hit Ratio** | `>= 94%` | `>= 90%` | `icx benchmark` |

---

## 2. Hot-Path Engineering Rules

1. **Zero Heap Allocations in HDC & Fractal Inner Loops:**
   - `Hypervector` (`[u64; 160]`) and `FractalCoord` (`[f64; 16]`) are stack-allocated `Copy` structs aligned to 64-byte cache lines (`#[repr(C, align(64))]`).
   - Hamming distance uses hardware `u64::count_ones()` (`POPCNT` / AVX-512 `VPOPCNTDQ`), which LLVM auto-vectorizes across the 160-element fixed array into `zmm` register loops.
2. **Single-Pass Structural Skeletonization:**
   - `StructuralSkeletonizer` scans input strings once by line slice without constructing intermediate AST heap trees or running backtracking regular expressions.
3. **Lock-Free Atomic & Safe Positional Shared Memory (`#![forbid(unsafe_code)]`):**
   - In `BackingStore::InProcess` mode, `SeqlockTelemetryBus` uses 100% lock-free `AtomicU32` and `AtomicU64` sequence counters (`Acquire`/`Release` memory ordering) over a fixed `256 x 4096`-byte cache-line-aligned slot array (`< 800 ns` per publish/poll, zero syscalls, zero mutexes).
   - In `BackingStore::SharedFile` (`/dev/shm/icx-fabric-v1.shm`) cross-process mode, to permanently uphold `#![forbid(unsafe_code)]` across all 8 workspace crates without `unsafe { libc::mmap(...) }`, `SeqlockTelemetryBus` uses safe positional file I/O (`std::os::unix::fs::FileExt::{read_exact_at, write_all_at}`) and OS advisory file locks (`File::try_lock()`), achieving `< 15 µs` uncontended cross-process propagation over the Linux `/dev/shm` tmpfs RAM disk.
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
