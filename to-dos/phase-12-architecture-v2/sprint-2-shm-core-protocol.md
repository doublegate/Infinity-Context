# Sprint 2 — `icx-shm` Core Protocol

**Phase:** Phase 12 — Architecture Release  
**Release:** v2.0.0  
**Sprint Goal:** Implement the hardened, pre-allocated mapping and the multi-writer seqlock
protocol (CAS ticket claims, atomic-word writes and reads with Release/Acquire ordering,
dead-writer recovery) with loss reporting at parity with the file bus.  
**Estimated Duration:** 2.5 weeks

## Research gate

Before any ticket starts:

- Re-read the accepted ADR-0007 (or its successor) and the layout spec from T-12-1-006.
- Re-check `posix_fallocate` behaviour on tmpfs (`ENOSPC` versus later SIGBUS) and on the
  filesystems that may back the fallback path.
- Re-check the current hardening code for the file bus (`open_shared_file` in `ring.rs`) so the
  mapping reuses the same checks.
- Re-read `proc_info` in `crates/icx-adapters/src/workspace.rs` (v1.3.2) and decide whether to
  move it to a shared crate.
- Confirm `libc`/`rustix` APIs for `O_NOFOLLOW`, `O_CLOEXEC`, `fstat` and `posix_fallocate` via
  Context7.

### Research notes

_None recorded yet._

## Tickets

### T-12-2-001 — Hardened open of the mapping

**Description:** Open the shm file with `O_NOFOLLOW|O_CLOEXEC` and owner-only mode (`0o600`),
creating it exclusively when absent.

**Research:** Confirm the flags on Linux and macOS and the race-free create-or-open sequence.

**Acceptance Criteria:**

- [ ] A symlink at the path is refused (tested).
- [ ] A created file has mode `0o600` regardless of umask (tested).
- [ ] The descriptor is not inherited across `exec` (tested).

**Dependencies:** T-12-1-008  
**Reference:** ADR-0007 §Decision 4; `crates/icx-bus/src/ring.rs` (`open_shared_file`)  
**Estimated Complexity:** S

---

### T-12-2-002 — `fstat` and header validation; refuse foreign or permissive files

**Description:** After open, `fstat` the descriptor to check owner uid, mode and size, then
validate the header (magic, format version, slot count, slot size, owner uid). Refuse foreign,
symlinked, permissive or mis-sized files and fall back to the file bus with a logged reason.

**Research:** Enumerate refusal cases from the layout spec and the file-bus hardening tests.

**Acceptance Criteria:**

- [ ] Each refusal case has a test and a distinct logged reason.
- [ ] Refusal falls back to the file bus; it never panics.

**Dependencies:** T-12-2-001, T-12-1-006  
**Reference:** ADR-0007 §Decision 4; `docs/security-and-threat-model.md`  
**Estimated Complexity:** M

---

### T-12-2-003 — Pre-allocate with `posix_fallocate` to avoid SIGBUS on full tmpfs

**Description:** Reserve the whole mapping with `posix_fallocate` before mapping it. A full tmpfs
otherwise delivers SIGBUS on a write to a sparse page instead of an error. With the reservation,
a full `/dev/shm` (Docker's default is 64 MB) becomes a clean fallback to the file bus.

**Research:** Hypothesis: after a successful `posix_fallocate`, writes into the mapping cannot
raise SIGBUS for lack of space. Confirming measurement: a test on a deliberately tiny tmpfs mount
(container or user namespace) where reservation fails cleanly and a reserved mapping survives
filling the rest of the mount. Rejection changes the fallback design before release.

**Acceptance Criteria:**

- [ ] Reservation failure (`ENOSPC`) falls back to the file bus with a logged reason (tested).
- [ ] No SIGBUS after filling the tmpfs once the mapping is reserved (tested on a small mount).

**Dependencies:** T-12-2-001  
**Reference:** Issue #32; ADR-0007 §Consequences (64 MB container `/dev/shm`);
`posix_fallocate(3)`  
**Estimated Complexity:** M

---

### T-12-2-004 — Multi-writer slot claim by compare-and-swap ticket

**Description:** Writers claim a slot without a lock by a compare-and-swap on the slot ticket;
a failed claim reports `SlotContended`, and a writer that would overwrite an unread lap reports
`SlotLapped`, as the file bus does.

**Research:** Confirm ticket semantics against the T-12-1-002 ordering argument.

**Acceptance Criteria:**

- [ ] Concurrent writers never write the same slot at once (tested in-process and multi-process).
- [ ] `SlotContended` and `SlotLapped` are returned in the same cases as the file bus.

**Dependencies:** T-12-2-002, T-12-1-002  
**Reference:** ADR-0007 §Decision 3; `crates/icx-bus/src/ring.rs` (`try_publish`)  
**Estimated Complexity:** M

---

### T-12-2-005 — Seqlock write: atomic data words, Release on the closing store

**Description:** The writer makes the sequence odd, stores the length and every payload word as
atomics, then stores the even closing sequence with Release ordering, with the fences the Boehm
analysis requires.

**Research:** Re-check the ordering argument for the opening store (a Release fence or
equivalent so data stores cannot move before it).

**Acceptance Criteria:**

- [ ] No plain (non-atomic) access to shared memory anywhere (reviewed; every access in a
  `// SAFETY:`-documented helper).
- [ ] Orderings match the committed ordering argument.

**Dependencies:** T-12-2-004  
**Reference:** Boehm 2012; Linux `seqcount_t`; ADR-0007 §Decision 2  
**Estimated Complexity:** L

---

### T-12-2-006 — Seqlock read with retry on odd or changed sequence

**Description:** The reader loads the sequence with Acquire, copies the atomic words, issues the
required fence, reloads the sequence, and retries when it was odd or changed, with a bounded
retry count that reports contention rather than spinning forever.

**Research:** Confirm the read-side fence placement from the ordering argument.

**Acceptance Criteria:**

- [ ] A reader never returns a payload whose checksum does not match (stress-tested in Sprint 3).
- [ ] Bounded retries are reported, not silently dropped.

**Dependencies:** T-12-2-005  
**Reference:** Boehm 2012; crossbeam `SeqLock`; ADR-0007 §Decision 2  
**Estimated Complexity:** L

---

### T-12-2-007 — Dead-writer recovery via PID plus start time

**Description:** A slot stuck odd past a deadline is recovered when the writer recorded in it is
no longer alive, checked by PID plus process start time through `/proc` (reusing the v1.3.2
`proc_info`), so PID reuse is not mistaken for liveness.

**Research:** Confirm `proc_info` gives start time on every Linux target, and the macOS
equivalent for Sprint 4.

**Acceptance Criteria:**

- [ ] A writer killed with SIGKILL mid-write leaves a slot that is recovered after the deadline
  (tested).
- [ ] A live writer is never recovered, including after PID reuse (tested).

**Dependencies:** T-12-2-005  
**Reference:** ADR-0007 §Decision 3; `crates/icx-adapters/src/workspace.rs` (`proc_info`)  
**Estimated Complexity:** M

---

### T-12-2-008 — Loss reporting at parity with the file bus

**Description:** Implement `SlotLapped`, `SlotContended`, `dropped_publishes` and
`poll_since_with_loss` for the shm backend with the same semantics as the file bus, through the
`BusBackend` trait.

**Research:** List the file bus's loss tests and confirm each is expressible against the trait.

**Acceptance Criteria:**

- [ ] The file bus's loss-reporting tests pass against the shm backend.
- [ ] Lost-event counts match between backends for the same publish and poll schedule (tested).

**Dependencies:** T-12-2-006, T-12-1-007  
**Reference:** Issue #32; `crates/icx-bus/src/ring.rs` (`dropped_publishes`,
`poll_since_with_loss`)  
**Estimated Complexity:** M

## Sprint exit

- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, with and without
  `--features shm-bus`.
- [ ] `cargo test --workspace` passes, with and without `--features shm-bus`.
- [ ] Coverage >= 95% (`cargo llvm-cov --workspace --all-targets --fail-under-lines 95`).
- [ ] Docs and `CHANGELOG.md` (`[Unreleased]`) updated in the same change.
- [ ] Upstream re-scan for Sprint 3 done, its tickets adjusted, and findings recorded in its
  Research notes.
