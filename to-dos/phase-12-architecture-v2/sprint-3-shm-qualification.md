# Sprint 3 — `icx-shm` Qualification

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Qualify the shm backend for release: `loom` models, a multi-process stress
harness on x86_64 and the arm64 runner, ThreadSanitizer and Miri within their scope, a fuzz
target, the whole bus suite against the shm backend, and a feature-off identity check. This
sprint is the release gate.  
**Estimated Duration:** 2.5 weeks

## Research gate

Before any ticket starts:

- Check the current `loom` version and its support for multi-word atomics and fences via
  Context7.
- Re-check what Miri and ThreadSanitizer can and cannot check (Miri cannot check memory-mapped
  files; TSan needs nightly and `-Zsanitizer=thread` with a rebuilt std).
- Confirm the Phase 9 `ubuntu-24.04-arm` runner is available and current; QEMU user mode is not an
  acceptable stand-in, since it does not model weak memory faithfully.
- Check the current `cargo-fuzz` version and the existing fuzz targets layout.

### Research notes

_None recorded yet._

## Tickets

### T-12-3-001 — `loom` models of claim, write, read and recovery

**Description:** Model the CAS claim, the seqlock write, the seqlock read and dead-writer
recovery orderings in `loom`, over the same code paths with `loom` atomics substituted.

**Research:** Confirm `loom` can model the fence pattern chosen in T-12-1-002, and the state-space
bound needed.

**Acceptance Criteria:**

- [ ] Models pass for two writers and one reader, and one writer and two readers.
- [ ] A deliberately weakened ordering makes a model fail (negative test).

**Dependencies:** T-12-2-004, T-12-2-005, T-12-2-006, T-12-2-007  
**Reference:** ADR-0007 §Decision 6; `loom` documentation  
**Estimated Complexity:** L

---

### T-12-3-002 — Multi-process stress harness

**Description:** N writers and readers as separate processes on one mapping, with checksummed
payloads for torn-read detection, SIGKILL of writers mid-write, wrap-around and lap detection.

**Research:** Choose N and run duration from a pilot so the run is long enough to exercise
wrap-around many times.

**Acceptance Criteria:**

- [ ] Zero checksum mismatches across the qualified run.
- [ ] Killed writers' slots are recovered; laps are reported, never silently lost.
- [ ] Parameters and outcome recorded in `docs/testing-strategy.md`.

**Dependencies:** T-12-2-008  
**Reference:** ADR-0007 §Decision 6; issue #32  
**Estimated Complexity:** L

---

### T-12-3-003 — Run the stress harness on x86_64 and the arm64 runner (not QEMU)

**Description:** Run T-12-3-002 in CI on x86_64 and on the `ubuntu-24.04-arm` runner, the
weak-memory hardware. QEMU user mode is not accepted as a substitute.

**Research:** Confirm runner minutes and timeouts allow the qualified duration.

**Acceptance Criteria:**

- [ ] Both architectures pass in CI.
- [ ] The workflow refuses to count a QEMU-emulated run as qualification.

**Dependencies:** T-12-3-002, T-93-006  
**Reference:** Plan Phase 12 Sprint 3; ADR-0007 §Consequences  
**Estimated Complexity:** M

---

### T-12-3-004 — ThreadSanitizer on the in-process paths

**Description:** Run the in-process shm paths under ThreadSanitizer on nightly.

**Research:** Confirm the TSan build recipe on the pinned nightly.

**Acceptance Criteria:**

- [ ] TSan job passes with no reports; it is pinned to a nightly date.

**Dependencies:** T-12-2-008  
**Reference:** Plan Phase 12 Sprint 3  
**Estimated Complexity:** S

---

### T-12-3-005 — Miri on everything that does not touch the mapping

**Description:** Run Miri on the header and slot decoding, the claim logic over a heap buffer, and
the backend selection; document what Miri cannot check (the mapping itself).

**Research:** Split the code so non-mapping logic is testable on a heap-backed stand-in.

**Acceptance Criteria:**

- [ ] Miri job passes; its scope is documented in `docs/testing-strategy.md`.

**Dependencies:** T-12-2-008  
**Reference:** ADR-0007 §Decision 6 ("Miri cannot check memory-mapped files")  
**Estimated Complexity:** M

---

### T-12-3-006 — cargo-fuzz target for slot and header decoding

**Description:** Fuzz the header and slot decoders with arbitrary bytes.

**Research:** Seed the corpus from valid headers and slots, and from each T-12-2-002 refusal case.

**Acceptance Criteria:**

- [ ] Fuzz target runs in CI for a fixed budget with zero crashes.
- [ ] Every malformed input yields a typed error, never a panic.

**Dependencies:** T-12-2-002  
**Reference:** ADR-0007 §Decision 6; module 60 (fuzz every untrusted-input parser)  
**Estimated Complexity:** S

---

### T-12-3-007 — Whole bus test suite against the shm backend

**Description:** Parameterise the existing bus tests over `BusBackend` so the complete suite runs
against the file, in-process and shm backends.

**Research:** Identify tests that assume a file-backed implementation detail and rewrite them
against the trait.

**Acceptance Criteria:**

- [ ] Every bus test passes on all three backends.

**Dependencies:** T-12-1-007, T-12-2-008  
**Reference:** Plan Phase 12 Sprint 3  
**Estimated Complexity:** M

---

### T-12-3-008 — Feature-off identity check against v1.9

**Description:** Verify that with `shm-bus` off, builds, tests, benchmarks and binaries are
identical to v1.9 apart from the version string and the documented refactor.

**Research:** Use the Phase 11 reproducible-build check to compare artifacts.

**Acceptance Criteria:**

- [ ] `cargo tree` for default features contains no `icx-shm` or `memmap2`.
- [ ] Default-feature test results and benchmarks match v1.9 within the noise rule.

**Dependencies:** T-12-1-010, T-11-7-006  
**Reference:** ADR-0007 §Consequences ("Unchanged"); module 50 (feature flags)  
**Estimated Complexity:** S

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, with and without
  `--features shm-bus`.
- [ ] `cargo test --workspace` passes, with and without `--features shm-bus`.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Qualification gate (T-12-3-001 to T-12-3-008) green; this is the release gate.
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 4 done, its tickets adjusted, and findings recorded in its
  Research notes.
