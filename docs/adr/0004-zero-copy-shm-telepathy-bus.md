# ADR 0004: Lock-Free `/dev/shm` Seqlock Ring for Cross-Agent Telepathy

## Status

Accepted (2026-11-16)

Superseded in part by ADR-0005 (v1.1.0).

## Context

Multiple agent processes (Claude Code, Codex, Antigravity CLI, Hermes) running concurrently on the same workstation need to share real-time working context (e.g., discovered bugs, file edits, failing assertions). Using SQLite or HTTP alone introduces lock contention (`SQLITE_BUSY`) or multi-millisecond serialization overhead inside synchronous hooks, and risks deadlocks if a hook process is killed mid-write (`SIGKILL`).

## Decision

We implement a POSIX shared-memory **Lock-Free Seqlock Ring Buffer** (`/dev/shm/icx-fabric-v1` or `.icx/icx-fabric-v1.shm`) in `crates/icx-bus` under `#![forbid(unsafe_code)]`:

- Fixed `256` slots of `4,096` bytes each (`1,048,640` bytes total including the `64`-byte `FabricHeader`).
- `FabricHeader` and `SeqlockSlot` are represented as 100% safe `AtomicU32` and `AtomicU64` word arrays (`#[repr(C, align(64))]`) with compile-time size and alignment assertions, and file-backed `/dev/shm` persistence uses safe positional I/O (`std::os::unix::fs::FileExt::{read_exact_at, write_all_at}`).
- Writers claim a slot via `AtomicU64::fetch_add` on the global sequence counter and bracket writes with odd/even `AcqRel`/`Release` updates on the per-slot seqlock counter.
- Readers execute wait-free optimistic reads validated by even sequence parity (`s1 == s2`).

## Consequences

- **Positive:** `< 300 ns` lock-free publish/poll latency for the in-process atomic ring (`BackingStore::InProcess`) and `< 10 µs` cross-process `/dev/shm` positional I/O + OS advisory lock transport (`BackingStore::SharedFile`); 100% safe Rust (`unsafe_code = "forbid"` across all 8 crates); immune to deadlocks even if an agent process crashes or is `SIGKILL`ed mid-write.
- **Negative:** Positional `AtomicU64` word packing and `pread`/`pwrite` syscalls on the file-backed `/dev/shm` path add a few microseconds over raw `mmap`, which remains well within the `< 100 µs` synchronous hook budget while guaranteeing 100% safe Rust.
