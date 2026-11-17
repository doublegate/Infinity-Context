# Phase 1 — Foundations, BLAKE3 CAS, LCM Merkle-DAG & `/dev/shm` Bus

## Goal

Establish the 8-crate Rust Cargo workspace, implement deterministic Content-Defined Chunking (`FastCDC`) and `BLAKE3` Content-Addressed Storage (`CAS`), build the Lossless Context Management (`LCM`) hierarchical Merkle-DAG and `CacheEpochController`, and implement the POSIX `/dev/shm` lock-free Seqlock Ring Buffer for sub-100µs cross-agent telepathy.

## Exit Criteria

- [x] Cargo workspace compiles cleanly with `cargo check --workspace --all-targets` and zero Clippy warnings.
- [x] `CasStore` chunks, hashes (`BLAKE3`), stores, and losslessly reconstructs arbitrary byte streams and 1-based line ranges with cryptographic integrity verification.
- [x] `MerkleContextDag` and `CacheEpochController` seal historical turns into immutable `CacheEpochNode` frames while maintaining `>= 94%` prefix stability.
- [x] `SeqlockRing` publishes and reads cross-agent `AHTL` packets over shared memory with zero lock contention.

## Sprints

- [Sprint 1 — Workspace, FastCDC & BLAKE3 CAS](sprint-1-workspace-and-cas.md) — Core types, chunker, and lossless CAS engine.
- [Sprint 2 — Lossless Merkle-DAG (`LCM`) & Cache-Epoch Controller](sprint-2-merkle-dag-and-epochs.md) — DAG hierarchy, sub-trajectory folding, and prefix-cache epoch pinning.
- [Sprint 3 — Lock-Free `/dev/shm` Seqlock Telepathy Bus](sprint-3-shm-seqlock-bus.md) — Shared-memory ring buffer and cross-agent broadcast protocol.

## Reference Docs

- [docs/architecture.md](../../docs/architecture.md)
- [docs/virtual-memory-and-dag-engine.md](../../docs/virtual-memory-and-dag-engine.md)
- [docs/wire-protocol-and-shm-bus.md](../../docs/wire-protocol-and-shm-bus.md)
