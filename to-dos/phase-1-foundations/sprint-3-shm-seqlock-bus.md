# Sprint 3 — Lock-Free `/dev/shm` Seqlock Telepathy Bus

**Phase:** Phase 1 — Foundations  
**Sprint Goal:** Implement the POSIX shared-memory lock-free `SeqlockRing` in `crates/icx-bus` enabling `< 100 µs` cross-agent state synchronization.  
**Estimated Duration:** 1 week

## Tickets

### T-13-001 — Implement `#[repr(C, align(64))]` `FabricHeader` & `SeqlockSlot` Layout

**Description:** Define the cache-line-aligned shared memory structures (`FabricHeader`, `SeqlockSlot`) with compile-time size and alignment assertions in `crates/icx-bus/src/ring.rs`.

**Acceptance Criteria:**
- [x] `size_of::<SeqlockSlot>() == 4096` and `align_of::<SeqlockSlot>() == 64`.
- [x] Magic header `ICX_SHM_MAGIC` and schema version are validated on attach.

**Dependencies:** T-11-002  
**Reference:** `docs/wire-protocol-and-shm-bus.md` §2.1  
**Estimated Complexity:** M

---

### T-13-002 — Implement Lock-Free Seqlock Writer & Wait-Free Optimistic Reader

**Description:** Implement `SeqlockTelemetryBus::publish` and `SeqlockTelemetryBus::poll_since` using `AtomicU64` odd/even sequence counters and `Acquire`/`Release` memory fences backed by `memmap2` or in-memory shared backing.

**Acceptance Criteria:**
- [x] Concurrent writers and readers execute with zero mutexes and never observe torn reads (`s1 == s2` invariant).
- [x] Multi-threaded stress test (8 writer threads, 8 reader threads, 10,000 messages) passes with zero corrupted slots.
- [x] Every `unsafe` block carries an explicit `// SAFETY:` invariant comment.

**Dependencies:** T-13-001  
**Reference:** `docs/wire-protocol-and-shm-bus.md` §2.2; `docs/adr/0004-zero-copy-shm-telepathy-bus.md`  
**Estimated Complexity:** L
