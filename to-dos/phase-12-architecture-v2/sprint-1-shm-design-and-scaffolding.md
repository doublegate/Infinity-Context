# Sprint 1 — `icx-shm` Design Acceptance and Scaffolding

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Settle the `icx-shm` design on current evidence, accept ADR-0007 (or supersede
it), specify the shared-memory layout, refactor the bus behind a `BusBackend` trait with no
behaviour change, and scaffold `crates/icx-shm` with its own lints, a CI lint gate, the `shm-bus`
feature and the `bus.backend` config key.  
**Estimated Duration:** 2 weeks

## Research gate

Before any ticket starts (each item is also a ticket below, so its outcome is recorded):

- Current `memmap2` API and safety contract via Context7.
- Seqlock memory-model literature: Hans-J. Boehm, "Can seqlocks get along with programming
  language memory models?" (MSPC 2012); Linux `seqcount_t` (`include/linux/seqlock.h`); crossbeam's
  `SeqLock`.
- The status of Rust RFC 3301 (`AtomicPerByte`, atomic memcpy).
- An audit of `shared_memory`, `raw_sync` and `shmem-ipc` for reuse; their `unsafe` would still
  be ours to audit.
- Re-read issue #32, ADR-0004, ADR-0007 and `crates/icx-bus/src/ring.rs` (`BusError::SlotLapped`,
  `BusError::SlotContended`, `dropped_publishes`, `poll_since_with_loss`).
- Re-run `cargo test --workspace` and the Phase 8 bus-timing traces; record the baseline.

### Research notes

_None recorded yet._

## Tickets

### T-12-1-001 — Research: current `memmap2` API and safety contract

**Description:** Confirm the current `memmap2` version, its `MmapMut::map_mut` safety
requirements, and whether it exposes what the design needs (shared mapping, flush, alignment).

**Research:** Hypothesis: `memmap2` is sufficient and no direct `mmap` call is needed.
Confirming evidence: every mapping operation in the layout spec maps onto a `memmap2` API.
Rejection records which calls go through `libc`/`rustix` directly.

**Acceptance Criteria:**

- [ ] Version, API surface and safety contract recorded in the Research notes.
- [ ] Decision on `memmap2` versus direct calls recorded for T-12-1-008.

**Dependencies:** None  
**Reference:** Issue #32; ADR-0007 §Context; `memmap2` documentation (Context7)  
**Estimated Complexity:** S

---

### T-12-1-002 — Research: seqlock memory-model literature

**Description:** Derive the required orderings from Boehm 2012, the Linux `seqcount_t`
implementation and crossbeam's `SeqLock`, for a multi-process, multi-writer ring whose data words
are all atomics.

**Research:** Hypothesis: Release on the closing sequence store, Acquire on the reader's sequence
loads, Relaxed atomic data words, and the fences Boehm identifies are sufficient. Confirming
evidence: the derivation plus a prototype `loom` model passing. Rejection updates the protocol in
T-12-1-006 before Sprint 2.

**Acceptance Criteria:**

- [ ] Written ordering argument for claim, write, read and recovery committed to
  `docs/wire-protocol-and-shm-bus.md`.
- [ ] Differences from crossbeam's `SeqLock` and `seqcount_t` listed with reasons.

**Dependencies:** None  
**Reference:** Boehm, "Can seqlocks get along with programming language memory models?" (2012);
Linux `include/linux/seqlock.h`; crossbeam `SeqLock`  
**Estimated Complexity:** M

---

### T-12-1-003 — Research: Rust RFC 3301 status

**Description:** Check whether RFC 3301 (`AtomicPerByte`, atomic memcpy) has landed or changed,
since it would replace the word-by-word atomic copy.

**Research:** Hypothesis: RFC 3301 is not stable for v2.0. Confirming evidence: current tracking
issue state. If it is stable, record how the payload copy would use it.

**Acceptance Criteria:**

- [ ] RFC status and the decision for the payload copy recorded.

**Dependencies:** None  
**Reference:** Rust RFC 3301; issue #32  
**Estimated Complexity:** S

---

### T-12-1-004 — Research: audit `shared_memory`, `raw_sync` and `shmem-ipc` for reuse

**Description:** Audit the three crates for maintenance, license, `unsafe` surface and fit.

**Research:** Hypothesis: none fits without carrying more `unsafe` than writing our own.
Confirming evidence: per-crate audit table. Rejection adopts the crate whose `unsafe` we can audit
and records that its `unsafe` is ours to review.

**Acceptance Criteria:**

- [ ] Audit table (maintenance, license, `unsafe` count, fit) in the Research notes.
- [ ] Reuse decision recorded in ADR-0007 or its successor.

**Dependencies:** None  
**Reference:** ADR-0007 (Alternatives considered); `shared_memory`, `raw_sync`, `shmem-ipc`  
**Estimated Complexity:** S

---

### T-12-1-005 — ADR-0007: proposed to accepted, or a superseding ADR

**Description:** Update ADR-0007 to `accepted` with the research outcomes, or write a
superseding ADR (linked both ways) if the research changes the design.

**Research:** Collect T-12-1-001 to T-12-1-004 outcomes.

**Acceptance Criteria:**

- [ ] ADR status `accepted` (or a new ADR supersedes it, with cross-links).
- [ ] Issue #32 updated with a link to the decision.

**Dependencies:** T-12-1-001, T-12-1-002, T-12-1-003, T-12-1-004  
**Reference:** `docs/adr/0007-opt-in-icx-shm-bus.md`; issue #32  
**Estimated Complexity:** S

---

### T-12-1-006 — Layout specification

**Description:** Specify the mapping in `docs/wire-protocol-and-shm-bus.md`. Header: magic, format
version, slot count, slot size, owner uid, creation time. Per slot: an `AtomicU64` sequence, a
CAS ticket, a length, and a payload held as `AtomicU64` words, never plain bytes. Include
alignment, endianness and versioning rules.

**Research:** Confirm alignment and size limits against the T-12-1-001 mapping API and the
current `FabricMemoryLayout`/`SeqlockSlot` sizes in `ring.rs`.

**Acceptance Criteria:**

- [ ] Byte-level layout table with offsets, sizes and atomic types.
- [ ] Version-mismatch and size-mismatch behaviour specified (refuse and fall back).

**Dependencies:** T-12-1-002, T-12-1-005  
**Reference:** ADR-0007 §Decision 2–3; `docs/wire-protocol-and-shm-bus.md`;
`crates/icx-bus/src/ring.rs`  
**Estimated Complexity:** M

---

### T-12-1-007 — Refactor `ring.rs` behind a `BusBackend` trait

**Description:** Introduce a `BusBackend` trait in `crates/icx-bus` and make the in-process and
file-backed buses implementations of it, with no behaviour change. Loss reporting
(`SlotLapped`, `SlotContended`, `dropped_publishes`, `poll_since_with_loss`) becomes part of the
trait contract.

**Research:** List every public `SeqlockTelemetryBus` method and its callers in `icx-adapters`
and `icx-cli`.

**Acceptance Criteria:**

- [ ] The full existing bus test suite passes unchanged.
- [ ] Publish and poll latency unchanged within noise: measured and published in
  `docs/numbers.md`.
- [ ] No public API break for default-feature users (`cargo-semver-checks`).

**Dependencies:** None  
**Reference:** Issue #32; `crates/icx-bus/src/ring.rs`  
**Estimated Complexity:** M

---

### T-12-1-008 — New crate `crates/icx-shm` with its own lints table

**Description:** Create `crates/icx-shm` with its own `[lints.rust]` table, `unsafe_code = "deny"`,
allowed only on audited items, with a `// SAFETY:` comment on every `unsafe` block. A crate
attribute cannot relax an inherited `forbid` (E0453), so the crate does not use
`[lints] workspace = true`.

**Research:** Confirm the lint table and `#[allow(unsafe_code)]` scoping behave as intended on the
current stable toolchain.

**Acceptance Criteria:**

- [ ] Crate builds, with `unsafe` denied except on explicitly allowed items.
- [ ] Every `unsafe` block carries a `// SAFETY:` comment (checked by
  `clippy::undocumented_unsafe_blocks` at deny).
- [ ] Workspace-wide clippy and the other lints still apply to the crate.

**Dependencies:** T-12-1-001  
**Reference:** ADR-0007 §Decision 1; module 50 (Safety)  
**Estimated Complexity:** S

---

### T-12-1-009 — CI gate: no other crate may drop the workspace lints

**Description:** A script in CI that fails if any crate other than `icx-shm` lacks
`[lints] workspace = true`, or if the workspace `unsafe_code = "forbid"` is weakened.

**Research:** Confirm the script parses `Cargo.toml` robustly (TOML parser, not grep).

**Acceptance Criteria:**

- [ ] Gate fails on a test fixture crate that drops the workspace lints, and passes on the tree.
- [ ] Runs as a blocking CI job.

**Dependencies:** T-12-1-008  
**Reference:** ADR-0007 §Decision 1  
**Estimated Complexity:** S

---

### T-12-1-010 — `shm-bus` feature and `bus.backend` config key

**Description:** Add the `shm-bus` feature to `icx-bus` and `icx-cli` (off by default), and the
`bus.backend = auto|file|shm` key in the v1.9 config file. `auto` selects the file bus in v2.0;
`shm` without the feature compiled in fails with a clear error.

**Research:** Confirm the v1.9 config loader handles enum keys and env override.

**Acceptance Criteria:**

- [ ] Default build has no `icx-shm` dependency in `cargo tree`.
- [ ] Each `bus.backend` value behaves as specified, with and without the feature (tested).

**Dependencies:** T-12-1-007, T-12-1-008, T-11-6-002  
**Reference:** Issue #32; ADR-0007 §Decision 5  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, with and without
  `--features shm-bus`.
- [ ] `cargo test --workspace` passes.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 2 done, its tickets adjusted, and findings recorded in its
  Research notes.
