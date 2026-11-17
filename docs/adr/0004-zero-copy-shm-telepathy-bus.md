# ADR 0004: Lock-Free `/dev/shm` Seqlock Ring for Cross-Agent Telepathy

## Status

Accepted (2026-11-16)

## Context

Multiple agent processes (Claude Code, Codex, Antigravity CLI, Hermes) running concurrently on the same workstation need to share real-time working context (e.g., discovered bugs, file edits, failing assertions). Using SQLite or HTTP alone introduces lock contention (`SQLITE_BUSY`) or multi-millisecond serialization overhead inside synchronous hooks, and risks deadlocks if a hook process is killed mid-write (`SIGKILL`).

## Decision

We implement a memory-mapped POSIX shared-memory **Lock-Free Seqlock Ring Buffer** (`/dev/shm/icx-fabric-v1` or `$XDG_RUNTIME_DIR/icx-fabric-v1.shm`) in `crates/icx-bus`:
- Fixed `4080` slots of `4,096` bytes each (`~16 MiB` total).
- Writers claim a slot via `AtomicU64::fetch_add` on the global sequence counter and bracket writes with odd/even `Release` stores on the per-slot seqlock counter.
- Readers execute wait-free optimistic reads validated by even sequence parity (`s1 == s2`).

## Consequences

- **Positive:** `< 1 microsecond` publish/poll latency; zero syscalls after `mmap`; immune to deadlocks even if an agent process crashes or is `SIGKILL`ed mid-write.
- **Negative:** Requires a small, carefully audited `unsafe` `mmap` module in `crates/icx-bus`, mitigated by `#[repr(C, align(64))]` layout assertions and Miri/concurrent stress tests.
